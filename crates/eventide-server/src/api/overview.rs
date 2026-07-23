//! Dashboard overview stats.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use eventide_core::AlertStatus;
use std::sync::Arc;

pub async fn overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let datasources = state.db.list_datasources().map_err(internal)?;
    let rules = state.db.list_rules().map_err(internal)?;
    let channels = state.db.list_channels().map_err(internal)?;
    let ingress = state.db.list_ingress_routes().map_err(internal)?;
    let alerts = state.db.list_alerts(None).map_err(internal)?;
    let silences = state.db.active_silences(chrono::Utc::now()).map_err(internal)?;

    let firing = alerts
        .iter()
        .filter(|a| a.status == AlertStatus::Firing)
        .count();
    let pending = alerts
        .iter()
        .filter(|a| a.status == AlertStatus::Pending)
        .count();
    let resolved = alerts
        .iter()
        .filter(|a| a.status == AlertStatus::Resolved)
        .count();
    let enabled_rules = rules.iter().filter(|r| r.enabled).count();

    Ok(Json(serde_json::json!({
        "datasources": datasources.len(),
        "rules": rules.len(),
        "enabled_rules": enabled_rules,
        "channels": channels.len(),
        "ingress_routes": ingress.len(),
        "alerts_total": alerts.len(),
        "alerts_firing": firing,
        "alerts_pending": pending,
        "alerts_resolved": resolved,
        "active_silences": silences.len(),
        "recent_alerts": alerts.into_iter().take(8).collect::<Vec<_>>(),
    })))
}

fn internal(e: impl std::fmt::Display) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}
