//! Dashboard overview stats.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

pub async fn overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let (firing, pending, resolved, total) =
        state.db.alert_status_counts().map_err(internal)?;
    let enabled_rules = state.db.count_enabled_rules().map_err(internal)?;
    let rules = state.db.count_table("rules").map_err(internal)?;
    let datasources = state.db.count_table("datasources").map_err(internal)?;
    let channels = state.db.count_table("notify_channels").map_err(internal)?;
    let ingress_routes = state.db.count_table("ingress_routes").map_err(internal)?;
    let active_silences = state
        .db
        .active_silences(chrono::Utc::now())
        .map_err(internal)?
        .len();
    let recent_alerts = state
        .db
        .list_recent_alerts_overview(10)
        .map_err(internal)?;
    let ingress = state.db.list_ingress_routes().map_err(internal)?;
    let ingress_brief: Vec<serde_json::Value> = ingress
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "kind": r.kind.as_str(),
                "enabled": r.enabled,
            })
        })
        .collect();
    let notify_skips = state
        .db
        .list_recent_notify_skips(6)
        .map_err(internal)?;
    let notify_skips: Vec<serde_json::Value> = notify_skips
        .into_iter()
        .map(|(error, transition, created_at)| {
            serde_json::json!({
                "error": error,
                "transition": transition,
                "created_at": created_at,
            })
        })
        .collect();

    let health = if firing > 0 {
        format!("{firing} 条正在告警")
    } else if pending > 0 {
        format!("无 firing · {pending} 条等待中")
    } else {
        "当前无正在告警".into()
    };
    let health = if active_silences > 0 {
        format!("{health} · {active_silences} 条静默生效")
    } else {
        health
    };

    Ok(Json(serde_json::json!({
        "health": health,
        "datasources": datasources,
        "rules": rules,
        "enabled_rules": enabled_rules,
        "channels": channels,
        "ingress_routes": ingress_routes,
        "alerts_total": total,
        "alerts_firing": firing,
        "alerts_pending": pending,
        "alerts_resolved": resolved,
        "active_silences": active_silences,
        "recent_alerts": recent_alerts,
        "ingress": ingress_brief,
        "notify_skips": notify_skips,
        "pressure_inflight": state.pressure.inflight(),
        "is_leader": state.leader.is_leader(),
        "leader_holder_id": state.leader.holder_id(),
    })))
}

fn internal(e: impl std::fmt::Display) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}
