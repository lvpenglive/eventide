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
mod leader;
mod notify_pipeline;
mod password;
mod scheduler;
mod state;
mod trap_proxy;

use anyhow::Context;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::db::Db;
use crate::scheduler::spawn_scheduler;
use crate::state::AppState;

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

    let throttle = eventide_core::RedisThrottleGate::new(
        config.storm.throttle_config(),
        &redis_client,
    )
    .context("redis throttle")?;
    let aggregate = eventide_core::RedisAggregateBuffer::new(
        config.storm.aggregate_config(),
        &redis_client,
    )
    .context("redis aggregate")?;
    let pressure = eventide_core::IngressPressure::new(config.storm.pressure_config());

    if config.storm.throttle_enabled {
        tracing::info!(
            min_interval = config.storm.min_interval_seconds,
            max_per_window = config.storm.max_per_window,
            window = config.storm.window_seconds,
            key = %config.storm.throttle_key,
            "storm throttle enabled (redis)"
        );
    }
    if config.storm.aggregate_enabled {
        tracing::info!(
            window = config.storm.aggregate_window_seconds,
            group_by = %config.storm.group_by,
            mode = %config.storm.aggregate_mode,
            "storm aggregate enabled (redis)"
        );
    }
    tracing::info!(
        max_inflight = config.storm.ingress_max_inflight,
        degrade_skip_notify = config.storm.degrade_skip_notify,
        degrade_notify_per_sec = config.storm.degrade_notify_per_sec,
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

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        notifier: eventide_notify::Notifier::new(),
        throttle,
        aggregate,
        pressure,
        leader,
        kafka_pool: Arc::new(eventide_sources::KafkaClientPool::new()),
        notify_queue,
        es: Arc::new(std::sync::RwLock::new(es_client)),
        alert_history: Arc::new(std::sync::RwLock::new(alert_history_prefs)),
    });
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
