//! Eventide HTTP API, scheduler, and static console.

mod api;
mod auth;
mod config;
mod db;
mod iam;
mod ingress_api;
mod kafka_ingress;
mod notify_pipeline;
mod password;
mod scheduler;
mod state;

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

    std::fs::create_dir_all(
        std::path::Path::new(&config.database_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )
    .ok();

    let db = Db::open(&config.database_path).context("open sqlite")?;
    db.migrate().context("migrate sqlite")?;
    db.seed_iam_if_empty(&config.auth.username, &config.auth.password)
        .context("seed IAM")?;

    let throttle = eventide_core::ThrottleGate::new(config.storm.throttle_config());
    let aggregate = eventide_core::AggregateBuffer::new(config.storm.aggregate_config());
    let pressure = eventide_core::IngressPressure::new(config.storm.pressure_config());
    if config.storm.throttle_enabled {
        tracing::info!(
            min_interval = config.storm.min_interval_seconds,
            max_per_window = config.storm.max_per_window,
            window = config.storm.window_seconds,
            key = %config.storm.throttle_key,
            "storm throttle enabled"
        );
    }
    if config.storm.aggregate_enabled {
        tracing::info!(
            window = config.storm.aggregate_window_seconds,
            group_by = %config.storm.group_by,
            mode = %config.storm.aggregate_mode,
            "storm aggregate enabled"
        );
    }
    tracing::info!(
        max_inflight = config.storm.ingress_max_inflight,
        degrade_skip_notify = config.storm.degrade_skip_notify,
        degrade_notify_per_sec = config.storm.degrade_notify_per_sec,
        "storm ingress pressure configured"
    );

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        notifier: eventide_notify::Notifier::new(),
        throttle,
        aggregate,
        pressure,
    });

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
