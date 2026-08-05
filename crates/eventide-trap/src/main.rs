//! Eventide Trap service: UDP SNMP Trap → normalize → Kafka.

mod config;
mod http;
mod kafka_out;
mod normalize;
mod parse;
mod stats;
mod usm;

use crate::config::TrapConfig;
use crate::http::AppState;
use crate::kafka_out::KafkaOut;
use crate::normalize::{to_ingress_alert_with_policy, to_kafka_payload_with_policy};
use crate::parse::parse_udp_datagram;
use crate::stats::{RecentBuffer, RecentItem, TrapStats};
use anyhow::{Context, Result};
use chrono::Utc;
use eventide_trap_data::{
    connect_mysql, ensure_trap_schema, run_resilient_pubsub, MibStore, PolicyRedis, PolicyStore,
    S3Store, TrapInstanceHeartbeat, POLICY_CHANGED_CHANNEL, TRAP_API_TOKEN_CHANNEL,
};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cfg_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "eventide-trap.toml".into());
    let config = TrapConfig::load(&cfg_path).unwrap_or_else(|e| {
        tracing::warn!("load `{cfg_path}` failed ({e:#}); using defaults");
        TrapConfig::default()
    });

    let kafka = KafkaOut::new(
        &config.kafka_brokers,
        config.kafka_topic.clone(),
        config.kafka_partitions,
        config.kafka_partition,
    );
    if kafka.is_none() {
        tracing::warn!("kafka_brokers empty — parse-only mode (no produce)");
    } else {
        tracing::info!(
            topic = %config.kafka_topic,
            partitions = config.kafka_partitions.max(1),
            fixed_partition = config.kafka_partition,
            "kafka produce enabled"
        );
    }

    let pool = connect_mysql(&config.mysql_url).context("mysql")?;
    ensure_trap_schema(&pool).context("trap schema")?;
    tracing::info!("MySQL ready (trap_policies / trap_mibs)");

    let s3 = S3Store::connect(
        &config.s3_endpoint,
        &config.s3_access_key,
        &config.s3_secret_key,
        &config.s3_region,
        &config.s3_bucket,
    )
    .await
    .context("s3 / rustfs")?;
    tracing::info!(
        endpoint = %config.s3_endpoint,
        bucket = %config.s3_bucket,
        "S3 / RustFS ready"
    );

    let mibs = Arc::new(
        MibStore::open(
            pool.clone(),
            s3,
            &config.mib_cache_dir,
            Some(Path::new(&config.mib_dir)),
        )
        .await
        .context("open MIB store")?,
    );
    tracing::info!(
        backend = %mibs.backend(),
        count = mibs.list().await.len(),
        "MIB library ready"
    );

    let policies = Arc::new(
        PolicyStore::open(pool.clone(), Some(Path::new(&config.policies_path)))
            .await
            .context("open policy store")?,
    );
    tracing::info!(
        backend = %policies.backend(),
        count = policies.list().await.len(),
        "trap policies ready (mysql)"
    );
    if config.snmpv3_users.is_empty() {
        tracing::info!("snmpv3_users empty — SNMPv3 traps rejected (v1/v2c ok)");
    } else {
        tracing::info!(
            count = config.snmpv3_users.len(),
            "snmpv3 USM users loaded"
        );
    }

    let policy_redis = if config.redis_url.trim().is_empty() {
        tracing::warn!("redis_url empty — policies load from MySQL only");
        None
    } else {
        match PolicyRedis::connect(&config.redis_url) {
            Ok(r) => {
                tracing::info!(redis = %config.redis_url, "trap policy redis ready");
                Some(Arc::new(r))
            }
            Err(e) => {
                tracing::warn!(error = %e, "trap policy redis unavailable; MySQL fallback");
                None
            }
        }
    };

    if let Some(r) = policy_redis.as_ref() {
        match policies.reload_prefer_redis(Some(r.as_ref())).await {
            Ok(()) => tracing::info!(
                count = policies.list().await.len(),
                stamp = %policies.current_stamp().await,
                "policies loaded (redis preferred)"
            ),
            Err(e) => tracing::warn!(error = %e, "initial redis/mysql policy load failed"),
        }
    }

    let mut api_token = config.api_token.clone();
    if let Some(r) = policy_redis.as_ref() {
        match r.load_api_token() {
            Ok(Some(t)) => {
                // Redis is source of truth when key exists (including empty = cleared).
                api_token = t;
                tracing::info!(
                    auth = !api_token.trim().is_empty(),
                    "trap api_token loaded from redis"
                );
            }
            Ok(None) => {
                if !api_token.trim().is_empty() {
                    // Seed redis from toml so console/server stay aligned.
                    if let Err(e) = r.publish_api_token(api_token.trim()) {
                        tracing::warn!(error = %e, "seed trap api_token to redis failed");
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "load trap api_token from redis failed"),
        }
    }
    let api_token = Arc::new(RwLock::new(api_token));

    let reload_secs = config.policy_reload_secs.max(1);
    let pol_bg = policies.clone();
    let mib_bg = mibs.clone();
    let redis_bg = policy_redis.clone();
    let token_bg = api_token.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(reload_secs));
        loop {
            tick.tick().await;
            match pol_bg
                .refresh_prefer_redis(redis_bg.as_deref())
                .await
            {
                Ok(true) => tracing::info!("policies reloaded (redis/mysql)"),
                Ok(false) => {}
                Err(e) => tracing::warn!("policy refresh: {e:#}"),
            }
            if let Some(r) = redis_bg.as_ref() {
                match r.load_api_token() {
                    Ok(Some(t)) => {
                        let auth = !t.trim().is_empty();
                        let changed = token_bg
                            .read()
                            .map(|g| g.as_str() != t)
                            .unwrap_or(true);
                        if changed {
                            if let Ok(mut g) = token_bg.write() {
                                *g = t;
                            }
                            tracing::info!(auth, "trap api_token refreshed (poll)");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::debug!(error = %e, "api_token poll skipped"),
                }
            }
            match mib_bg.refresh_if_changed().await {
                Ok(true) => tracing::info!("MIBs reloaded from MySQL/S3"),
                Ok(false) => {}
                Err(e) => tracing::warn!("MIB refresh: {e:#}"),
            }
        }
    });

    if let Some(r) = policy_redis.clone() {
        spawn_change_pubsub(r, policies.clone(), api_token.clone());
    }

    let state = Arc::new(AppState {
        recent: RecentBuffer::new(config.recent_limit),
        stats: TrapStats::default(),
        kafka,
        mibs,
        policies,
        policy_redis,
        api_token: api_token.clone(),
        config: config.clone(),
    });

    let udp_state = state.clone();
    let udp_addr: SocketAddr = config.listen_udp.parse().context("parse listen_udp")?;
    tokio::spawn(async move {
        if let Err(e) = run_udp(udp_addr, udp_state).await {
            tracing::error!("udp listener stopped: {e:#}");
        }
    });

    let instance_id = config.resolve_instance_id();
    if config.heartbeat_secs > 0 {
        if let Some(r) = state.policy_redis.clone() {
            spawn_instance_heartbeat(
                r,
                state.clone(),
                instance_id.clone(),
                config.heartbeat_secs,
            );
            tracing::info!(
                %instance_id,
                heartbeat_secs = config.heartbeat_secs,
                ha_vip = %config.ha_vip,
                "trap instance heartbeat enabled"
            );
        } else {
            tracing::warn!("heartbeat_secs set but redis unavailable — HA registry disabled");
        }
    }

    let http_addr: SocketAddr = config.listen_http.parse().context("parse listen_http")?;
    let app = http::router(state);
    tracing::info!(%instance_id, "eventide-trap http://{http_addr}");
    tracing::info!("eventide-trap udp://{udp_addr}");
    if !config.ha_vip.trim().is_empty() {
        tracing::info!(vip = %config.ha_vip, "ha_vip advertised (bind VIP via Keepalived on MASTER)");
    }
    let auth_on = api_token
        .read()
        .map(|g| !g.trim().is_empty())
        .unwrap_or(false);
    if auth_on {
        tracing::info!("Trap HTTP auth enabled (Bearer / X-Eventide-Trap-Token)");
    } else {
        tracing::warn!("api_token empty — Trap HTTP (except health) is open");
    }
    tracing::info!("MIB/policy CRUD is on eventide-server (/api/mibs, /api/policies)");

    let listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .with_context(|| format!("bind http {http_addr}"))?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_instance_heartbeat(
    redis: Arc<PolicyRedis>,
    state: Arc<AppState>,
    instance_id: String,
    interval_secs: u64,
) {
    let interval = Duration::from_secs(interval_secs.max(1));
    let ttl = interval_secs.max(1).saturating_mul(3);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        let mut fail_streak = 0u32;
        loop {
            tick.tick().await;
            let hb = TrapInstanceHeartbeat {
                instance_id: instance_id.clone(),
                listen_udp: state.config.listen_udp.clone(),
                listen_http: state.config.listen_http.clone(),
                ha_vip: state.config.ha_vip.clone(),
                kafka_topic: state.config.kafka_topic.clone(),
                received: state.stats.received.load(Ordering::Relaxed),
                parsed_ok: state.stats.parsed_ok.load(Ordering::Relaxed),
                updated_at: Utc::now().to_rfc3339(),
            };
            match redis.beat_instance(&hb, ttl) {
                Ok(()) => fail_streak = 0,
                Err(e) => {
                    fail_streak = fail_streak.saturating_add(1);
                    if fail_streak == 1 || fail_streak % 12 == 0 {
                        tracing::warn!(
                            error = %e,
                            fail_streak,
                            "trap instance heartbeat failed"
                        );
                    } else {
                        tracing::debug!(error = %e, fail_streak, "trap instance heartbeat failed");
                    }
                }
            }
        }
    });
}

/// One Redis connection for policy + api_token channels (fewer sockets through NAT).
fn spawn_change_pubsub(
    redis: Arc<PolicyRedis>,
    policies: Arc<PolicyStore>,
    api_token: Arc<RwLock<String>>,
) {
    let policy_notify = Arc::new(Notify::new());
    let token_notify = Arc::new(Notify::new());
    let policy_ready = policy_notify.clone();
    let token_ready = token_notify.clone();
    let client = redis.client().clone();

    std::thread::Builder::new()
        .name("trap-redis-pubsub".into())
        .spawn(move || {
            let channels = [POLICY_CHANGED_CHANNEL, TRAP_API_TOKEN_CHANNEL];
            run_resilient_pubsub(
                &client,
                &channels,
                || {
                    policy_ready.notify_one();
                    token_ready.notify_one();
                },
                |ch| {
                    if ch == POLICY_CHANGED_CHANNEL {
                        policy_ready.notify_one();
                    } else if ch == TRAP_API_TOKEN_CHANNEL {
                        token_ready.notify_one();
                    } else {
                        policy_ready.notify_one();
                        token_ready.notify_one();
                    }
                },
            );
        })
        .expect("spawn trap redis pubsub thread");

    let redis_pol = redis.clone();
    let policies_bg = policies;
    tokio::spawn(async move {
        loop {
            policy_notify.notified().await;
            loop {
                tokio::select! {
                    biased;
                    _ = policy_notify.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(80)) => break,
                }
            }
            match policies_bg
                .reload_prefer_redis(Some(redis_pol.as_ref()))
                .await
            {
                Ok(()) => {
                    let count = policies_bg.list().await.len();
                    tracing::info!(count, "policies reloaded via redis pubsub");
                }
                Err(e) => tracing::warn!(error = %e, "pubsub policy reload failed"),
            }
        }
    });

    let redis_tok = redis;
    tokio::spawn(async move {
        loop {
            token_notify.notified().await;
            loop {
                tokio::select! {
                    biased;
                    _ = token_notify.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(80)) => break,
                }
            }
            match redis_tok.load_api_token() {
                Ok(Some(t)) => {
                    let auth = !t.trim().is_empty();
                    if let Ok(mut g) = api_token.write() {
                        *g = t;
                    }
                    tracing::info!(auth, "trap api_token reloaded via redis pubsub");
                }
                Ok(None) => {
                    if let Ok(mut g) = api_token.write() {
                        g.clear();
                    }
                    tracing::info!("trap api_token cleared via redis (key missing)");
                }
                Err(e) => tracing::warn!(error = %e, "api_token reload failed"),
            }
        }
    });
}

async fn run_udp(addr: SocketAddr, state: Arc<AppState>) -> Result<()> {
    let sock = tokio::net::UdpSocket::bind(addr)
        .await
        .with_context(|| format!("bind udp {addr}"))?;
    tracing::info!("listening SNMP Trap on udp://{addr}");
    let mut buf = vec![0u8; 65535];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        state.stats.received.fetch_add(1, Ordering::Relaxed);
        let datagram = &buf[..n];
        match parse_udp_datagram(
            datagram,
            peer,
            &state.config.community,
            &state.config.snmpv3_users,
        ) {
            Ok(trap) => {
                state.stats.parsed_ok.fetch_add(1, Ordering::Relaxed);
                if trap.version == "v3" {
                    state.stats.v3_ok.fetch_add(1, Ordering::Relaxed);
                }
                let policy = state.policies.match_trap(&trap).await;
                let alert = to_ingress_alert_with_policy(&trap, &state.config, policy.as_ref());
                let payload = to_kafka_payload_with_policy(&trap, &state.config, policy.as_ref());
                let mut kafka_ok = false;
                if let Some(k) = &state.kafka {
                    match k.publish(&trap.peer_ip, payload).await {
                        Ok(()) => {
                            state.stats.kafka_ok.fetch_add(1, Ordering::Relaxed);
                            kafka_ok = true;
                        }
                        Err(e) => {
                            state.stats.kafka_err.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(%peer, "kafka publish: {e:#}");
                        }
                    }
                }
                let alertname = alert
                    .get("labels")
                    .and_then(|l| l.get("alertname"))
                    .and_then(|x| x.as_str())
                    .unwrap_or(&trap.alertname)
                    .to_string();
                state
                    .recent
                    .push(RecentItem {
                        at: Utc::now().to_rfc3339(),
                        peer: trap.peer_ip.clone(),
                        trap_oid: trap.trap_oid.clone(),
                        alertname,
                        kafka: kafka_ok,
                        alert,
                    })
                    .await;
            }
            Err(e) => {
                state.stats.parse_err.fetch_add(1, Ordering::Relaxed);
                let es = e.to_string();
                if es.contains("usm") || es.contains("snmpv3") || es.contains("authentication") {
                    state.stats.v3_auth_fail.fetch_add(1, Ordering::Relaxed);
                }
                tracing::debug!(%peer, "snmp parse: {e}");
            }
        }
    }
}
