//! Dashboard overview stats.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::Utc;
use std::sync::Arc;

pub async fn overview(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let s_alerts = state.clone();
    let s_meta = state.clone();
    let s_cluster = state.clone();

    // 远端 MySQL / Redis RTT 并行，避免串行叠加。
    let (alerts_res, meta_res, cluster_res) = tokio::join!(
        tokio::task::spawn_blocking(move || collect_alert_slice(&s_alerts)),
        tokio::task::spawn_blocking(move || collect_meta_slice(&s_meta)),
        tokio::task::spawn_blocking(move || crate::instance_registry::cluster_snapshot(&s_cluster)),
    );

    let alerts = alerts_res.map_err(|e| internal(e))?.map_err(internal)?;
    let meta = meta_res.map_err(|e| internal(e))?.map_err(internal)?;
    let cluster = cluster_res.map_err(|e| internal(e))?;

    let mut health = if alerts.firing > 0 {
        format!("{} 条正在告警", alerts.firing)
    } else if alerts.pending > 0 {
        format!("无 firing · {} 条等待中", alerts.pending)
    } else {
        "当前无正在告警".into()
    };
    if meta.active_silences > 0 {
        health = format!("{health} · {} 条静默生效", meta.active_silences);
    }
    if meta.active_maintenance > 0 {
        health = format!("{health} · {} 个维护窗生效", meta.active_maintenance);
    }

    Ok(Json(serde_json::json!({
        "health": health,
        "datasources": meta.datasources,
        "rules": meta.rules,
        "enabled_rules": meta.enabled_rules,
        "channels": meta.channels,
        "ingress_routes": meta.ingress_routes,
        "alerts_total": alerts.total,
        "alerts_firing": alerts.firing,
        "alerts_pending": alerts.pending,
        "alerts_resolved": alerts.resolved,
        "active_silences": meta.active_silences,
        "active_maintenance_windows": meta.active_maintenance,
        "recent_alerts": alerts.recent_alerts,
        "ingress": meta.ingress_brief,
        "pressure_inflight": state.pressure.inflight(),
        "ingress_max_inflight": state
            .storm
            .read()
            .map(|s| s.ingress_max_inflight)
            .unwrap_or(state.config.storm.ingress_max_inflight),
        "is_leader": state.leader.is_leader(),
        "leader_holder_id": state.leader.holder_id(),
        "cluster_enabled": state.config.cluster.enabled,
        "cluster": cluster,
        "metrics": state.metrics.snapshot_json(&state),
        "metrics_endpoint": "/metrics",
    })))
}

struct AlertSlice {
    firing: usize,
    pending: usize,
    resolved: usize,
    total: usize,
    recent_alerts: Vec<eventide_core::AlertEvent>,
}

struct MetaSlice {
    enabled_rules: usize,
    rules: usize,
    datasources: usize,
    channels: usize,
    ingress_routes: usize,
    active_silences: usize,
    active_maintenance: usize,
    ingress_brief: Vec<serde_json::Value>,
}

fn collect_alert_slice(state: &AppState) -> anyhow::Result<AlertSlice> {
    let (firing, pending, resolved, total) = state.db.alert_status_counts()?;
    let recent_alerts = state.db.list_recent_alerts_overview(10)?;
    Ok(AlertSlice {
        firing,
        pending,
        resolved,
        total,
        recent_alerts,
    })
}

fn collect_meta_slice(state: &AppState) -> anyhow::Result<MetaSlice> {
    let now = Utc::now();
    let counts = state.db.overview_meta_counts(now)?;
    let ingress = state.db.list_ingress_routes()?;
    let ingress_brief = ingress
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
    Ok(MetaSlice {
        enabled_rules: counts.enabled_rules,
        rules: counts.rules,
        datasources: counts.datasources,
        channels: counts.channels,
        ingress_routes: counts.ingress_routes,
        active_silences: counts.active_silences,
        active_maintenance: counts.active_maintenance,
        ingress_brief,
    })
}

fn internal(e: impl std::fmt::Display) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}
