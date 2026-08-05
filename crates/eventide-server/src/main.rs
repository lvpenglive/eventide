//! Eventide HTTP API, scheduler, and static console.

mod api;
mod alert_history;
mod auth;
mod config;
mod db;
mod elasticsearch;
mod iam;
mod ingress_api;
mod kafka_ingress;
mod kafka_groups;
mod leader;
mod license;
mod notify_pipeline;
mod password;
mod scheduler;
mod state;
mod trap_proxy;
mod trap_token;
mod storm_sync;

use anyhow::Context;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::{AppConfig, TrapProxyConfig};
use crate::db::Db;
use crate::scheduler::spawn_scheduler;
use crate::state::AppState;
use eventide_trap_data::{MibStore, PolicyRedis, PolicyStore, S3Store};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "eventide.toml".to_string());
    let config = AppConfig::load(&config_path)
        .with_context(|| format!("load config from {config_path}"))?;

    let db = Db::connect(&config.mysql_url).context("connect mysql")?;
    db.migrate().context("migrate mysql")?;
    db.seed_iam_if_empty(&config.auth.username, &config.auth.password)
        .context("seed IAM")?;

    let redis_client =
        redis::Client::open(config.redis_url.as_str()).context("parse redis_url")?;
    // Fail fast if Redis is unreachable.
    {
        let mut conn = redis_client
            .get_connection()
            .context("connect redis")?;
        let pong: String = redis::cmd("PING")
            .query(&mut conn)
            .context("redis PING")?;
        tracing::info!(%pong, redis = %config.redis_url, "redis ready");
    }

    let storm_runtime = {
        use crate::state::STORM_CONFIG_KEY;
        match db.get_kv(STORM_CONFIG_KEY) {
            Ok(Some(s)) => match serde_json::from_str::<crate::config::StormConfig>(&s) {
                Ok(c) => {
                    tracing::info!("storm config loaded from app_kv (console)");
                    c.normalize()
                }
                Err(e) => {
                    tracing::warn!(error = %e, "storm app_kv invalid; using eventide.toml [storm]");
                    config.storm.clone()
                }
            },
            _ => config.storm.clone(),
        }
    };

    let throttle = eventide_core::RedisThrottleGate::new(
        storm_runtime.throttle_config(),
        &redis_client,
    )
    .context("redis throttle")?;
    let aggregate = eventide_core::RedisAggregateBuffer::new(
        storm_runtime.aggregate_config(),
        &redis_client,
    )
    .context("redis aggregate")?;
    let pressure = eventide_core::IngressPressure::new(storm_runtime.pressure_config());

    if storm_runtime.throttle_enabled {
        tracing::info!(
            min_interval = storm_runtime.min_interval_seconds,
            max_per_window = storm_runtime.max_per_window,
            window = storm_runtime.window_seconds,
            key = %storm_runtime.throttle_key,
            "storm throttle enabled (redis)"
        );
    }
    if storm_runtime.aggregate_enabled {
        tracing::info!(
            window = storm_runtime.aggregate_window_seconds,
            group_by = %storm_runtime.group_by,
            mode = %storm_runtime.aggregate_mode,
            "storm aggregate enabled (redis)"
        );
    }
    tracing::info!(
        max_inflight = storm_runtime.ingress_max_inflight,
        degrade_skip_notify = storm_runtime.degrade_skip_notify,
        degrade_notify_per_sec = storm_runtime.degrade_notify_per_sec,
        "storm ingress pressure configured"
    );
    tracing::info!(mysql = %config.mysql_url, "mysql ready");

    let leader = Arc::new(
        crate::leader::LeaderElection::new(&redis_client, &config.cluster)
            .context("leader election")?,
    );
    tracing::info!(
        enabled = config.cluster.enabled,
        key = %config.cluster.leader_key,
        lease_seconds = config.cluster.lease_seconds,
        holder = %leader.holder_id(),
        "cluster leader election configured"
    );
    crate::leader::spawn_leader_loop(leader.clone());

    let (notify_queue, notify_rx) = crate::notify_pipeline::NotifyQueue::new();

    let alert_history_prefs = {
        use crate::alert_history::{AlertHistoryPrefs, ALERT_HISTORY_KEY};
        let c = &config.elasticsearch;
        let prefs = match db.get_kv(ALERT_HISTORY_KEY) {
            Ok(Some(s)) => serde_json::from_str::<AlertHistoryPrefs>(&s).unwrap_or_default(),
            _ => AlertHistoryPrefs::default(),
        }
        .with_toml_fallback(&c.url, &c.index, &c.username, &c.password);
        tracing::info!(
            write_to_es = prefs.write_to_es,
            search_store = %prefs.search_store,
            es_url = %prefs.es_url,
            "alert history prefs loaded"
        );
        prefs
    };
    let es_client = match alert_history_prefs.build_es_client() {
        Ok(c) => {
            if c.is_some() {
                tracing::info!(
                    url = %alert_history_prefs.es_url,
                    index = %alert_history_prefs.es_index,
                    "elasticsearch client ready"
                );
            }
            c
        }
        Err(e) => {
            tracing::warn!(error = %e, "elasticsearch client init failed");
            None
        }
    };

    let (mibs, policies) = open_trap_stores(&db, &config.trap).await;

    let policy_redis = match PolicyRedis::connect(&config.redis_url) {
        Ok(r) => {
            tracing::info!("trap policy redis publisher ready");
            Some(Arc::new(r))
        }
        Err(e) => {
            tracing::warn!(error = %e, "trap policy redis publisher unavailable");
            None
        }
    };

    let trap_api_token_runtime = {
        use crate::trap_token::{TrapApiTokenPrefs, TRAP_API_TOKEN_KEY};
        match db.get_kv(TRAP_API_TOKEN_KEY) {
            Ok(Some(s)) => serde_json::from_str::<TrapApiTokenPrefs>(&s)
                .map(|p| p.token)
                .unwrap_or_default(),
            _ => String::new(),
        }
    };
    if !trap_api_token_runtime.trim().is_empty() {
        tracing::info!("trap api_token loaded from app_kv (console)");
    } else if !config.trap.api_token.trim().is_empty() {
        tracing::info!("trap api_token from eventide.toml [trap]");
    } else {
        tracing::warn!("trap api_token empty — /trap-api upstream auth off until set in 系统设置");
    }

    let license = crate::license::LicenseGate::from_db(&db).context("evaluate product license")?;

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        notifier: eventide_notify::Notifier::new(),
        throttle,
        aggregate,
        pressure,
        storm: Arc::new(std::sync::RwLock::new(storm_runtime)),
        leader,
        kafka_pool: Arc::new(eventide_sources::KafkaClientPool::new()),
        notify_queue,
        es: Arc::new(std::sync::RwLock::new(es_client)),
        alert_history: Arc::new(std::sync::RwLock::new(alert_history_prefs)),
        mibs,
        policies,
        policy_redis,
        trap_api_token: Arc::new(std::sync::RwLock::new(trap_api_token_runtime)),
        license,
    });
    // Seed Redis snapshot so Trap instances can boot without waiting for a CRUD.
    state.sync_policies_to_redis().await;
    state.sync_trap_api_token_to_redis();
    // Seed storm override (or clear) so peers / late joiners see the same prefs.
    {
        use crate::state::STORM_CONFIG_KEY;
        let reset = !matches!(state.db.get_kv(STORM_CONFIG_KEY), Ok(Some(_)));
        state.sync_storm_to_redis(reset);
    }
    crate::storm_sync::spawn_storm_config_pubsub(state.clone());
    crate::notify_pipeline::spawn_notify_workers(state.clone(), notify_rx);

    spawn_scheduler(state.clone());
    crate::kafka_ingress::spawn_kafka_ingress(state.clone());
    crate::notify_pipeline::spawn_aggregate_flusher(state.clone());

    let static_dir = config.static_dir.clone();
    let app = Router::new()
        .merge(api::router(state.clone()))
        .fallback_service(ServeDir::new(&static_dir))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = config.listen.parse().context("parse listen addr")?;
    tracing::info!("eventide listening on http://{addr}");
    tracing::info!("static console from {static_dir}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn open_trap_stores(
    db: &Db,
    trap: &TrapProxyConfig,
) -> (Option<Arc<MibStore>>, Option<Arc<PolicyStore>>) {
    let pool = db.pool();
    let policies = match PolicyStore::open(pool.clone(), None).await {
        Ok(p) => {
            tracing::info!(backend = %p.backend(), count = p.list().await.len(), "trap policies ready");
            Some(Arc::new(p))
        }
        Err(e) => {
            tracing::warn!(error = %e, "trap policy store unavailable");
            None
        }
    };

    if !trap.s3_configured() {
        tracing::warn!("[trap] s3_* not configured — MIB CRUD disabled");
        return (None, policies);
    }

    let mibs = match S3Store::connect(
        &trap.s3_endpoint,
        &trap.s3_access_key,
        &trap.s3_secret_key,
        &trap.s3_region,
        &trap.s3_bucket,
    )
    .await
    {
        Ok(s3) => match MibStore::open(pool, s3, &trap.mib_cache_dir, None).await {
            Ok(m) => {
                tracing::info!(
                    backend = %m.backend(),
                    count = m.list().await.len(),
                    "MIB library ready on server"
                );
                Some(Arc::new(m))
            }
            Err(e) => {
                tracing::warn!(error = %e, "MIB store open failed");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "S3 / RustFS connect failed — MIB CRUD disabled");
            None
        }
    };

    (mibs, policies)
}
