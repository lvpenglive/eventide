//! Eventide Trap service: UDP SNMP Trap → normalize → Kafka.

mod config;
mod db;
mod http;
mod kafka_out;
mod mib_api;
mod mib_store;
mod normalize;
mod parse;
mod policy_api;
mod policy_store;
mod policy_xlsx;
mod s3_store;
mod stats;

use crate::config::TrapConfig;
use crate::http::AppState;
use crate::kafka_out::KafkaOut;
use crate::mib_store::MibStore;
use crate::normalize::{to_ingress_alert_with_policy, to_kafka_payload_with_policy};
use crate::parse::parse_udp_datagram;
use crate::policy_store::PolicyStore;
use crate::s3_store::S3Store;
use crate::stats::{RecentBuffer, RecentItem, TrapStats};
use anyhow::{Context, Result};
use chrono::Utc;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
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
        config.kafka_partition,
    );
    if kafka.is_none() {
        tracing::warn!("kafka_brokers empty — parse-only mode (no produce)");
    } else {
        tracing::info!(
            topic = %config.kafka_topic,
            partition = config.kafka_partition,
            "kafka produce enabled"
        );
    }

    let pool = db::connect(&config.mysql_url).context("mysql")?;
    db::ensure_trap_schema(&pool).context("trap schema")?;
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
        "trap policies ready"
    );

    let reload_secs = config.policy_reload_secs.max(1);
    let pol_bg = policies.clone();
    let mib_bg = mibs.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(reload_secs));
        loop {
            tick.tick().await;
            match pol_bg.refresh_if_changed().await {
                Ok(true) => tracing::info!("policies reloaded from MySQL"),
                Ok(false) => {}
                Err(e) => tracing::warn!("policy refresh: {e:#}"),
            }
            match mib_bg.refresh_if_changed().await {
                Ok(true) => tracing::info!("MIBs reloaded from MySQL/S3"),
                Ok(false) => {}
                Err(e) => tracing::warn!("MIB refresh: {e:#}"),
            }
        }
    });

    let state = Arc::new(AppState {
        recent: RecentBuffer::new(config.recent_limit),
        stats: TrapStats::default(),
        kafka,
        mibs,
        policies,
        config: config.clone(),
    });

    let udp_state = state.clone();
    let udp_addr: SocketAddr = config.listen_udp.parse().context("parse listen_udp")?;
    tokio::spawn(async move {
        if let Err(e) = run_udp(udp_addr, udp_state).await {
            tracing::error!("udp listener stopped: {e:#}");
        }
    });

    let http_addr: SocketAddr = config.listen_http.parse().context("parse listen_http")?;
    let app = http::router(state);
    tracing::info!("eventide-trap http://{http_addr}");
    tracing::info!("eventide-trap udp://{udp_addr}");
    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_udp(addr: SocketAddr, state: Arc<AppState>) -> Result<()> {
    let sock = tokio::net::UdpSocket::bind(addr)
        .await
        .with_context(|| format!("bind udp {addr}"))?;
    tracing::info!("listening for SNMP traps on udp://{addr}");
    let mut buf = vec![0u8; 65535];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        state.stats.received.fetch_add(1, Ordering::Relaxed);
        let data = &buf[..n];
        match parse_udp_datagram(data, peer, &state.config.community) {
            Ok(trap) => {
                state.stats.parsed_ok.fetch_add(1, Ordering::Relaxed);
                let policy = state.policies.match_trap(&trap).await;
                let alert =
                    to_ingress_alert_with_policy(&trap, &state.config, policy.as_ref());
                let payload =
                    to_kafka_payload_with_policy(&trap, &state.config, policy.as_ref());
                let mut kafka_ok = false;
                if let Some(k) = &state.kafka {
                    match k.publish(&trap.peer_ip, payload).await {
                        Ok(()) => {
                            state.stats.kafka_ok.fetch_add(1, Ordering::Relaxed);
                            kafka_ok = true;
                        }
                        Err(e) => {
                            state.stats.kafka_err.fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(%peer, "kafka produce failed: {e:#}");
                        }
                    }
                }
                let alertname = alert
                    .get("labels")
                    .and_then(|l| l.get("alertname"))
                    .and_then(|x| x.as_str())
                    .unwrap_or(trap.alertname.as_str());
                tracing::info!(
                    %peer,
                    oid = %trap.trap_oid,
                    name = %alertname,
                    policy = policy.as_ref().map(|p| p.name.as_str()).unwrap_or("-"),
                    kafka = kafka_ok,
                    "trap accepted"
                );
                state
                    .recent
                    .push(RecentItem {
                        at: Utc::now().to_rfc3339(),
                        peer: trap.peer_ip,
                        trap_oid: trap.trap_oid,
                        alertname: alertname.to_string(),
                        kafka: kafka_ok,
                        alert,
                    })
                    .await;
            }
            Err(e) => {
                state.stats.parse_err.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(%peer, "trap parse skip: {e}");
            }
        }
    }
}
