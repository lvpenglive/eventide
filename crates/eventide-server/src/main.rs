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

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        notifier: eventide_notify::Notifier::new(),
    });

    spawn_scheduler(state.clone());
    crate::kafka_ingress::spawn_kafka_ingress(state.clone());

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
