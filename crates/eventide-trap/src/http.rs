//! HTTP API for health, stats, simulate, recent.

use crate::config::TrapConfig;
use crate::kafka_out::KafkaOut;
use crate::normalize::{simulate_parsed, to_ingress_alert_with_policy, to_kafka_payload_with_policy};
use crate::stats::{RecentBuffer, RecentItem, TrapStats};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub config: TrapConfig,
    pub stats: TrapStats,
    pub recent: RecentBuffer,
    pub kafka: Option<KafkaOut>,
    pub mibs: Arc<crate::mib_store::MibStore>,
    pub policies: Arc<crate::policy_store::PolicyStore>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/stats", get(stats))
        .route("/api/recent", get(recent))
        .route("/api/simulate", post(simulate))
        .merge(crate::mib_api::routes())
        .merge(crate::policy_api::routes())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "eventide-trap",
        "listen_udp": st.config.listen_udp,
        "kafka_enabled": st.kafka.is_some(),
        "kafka_topic": st.config.kafka_topic,
        "policy_count": st.policies.list().await.len(),
    }))
}

async fn stats(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(st.stats.snapshot())
}

async fn recent(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "items": st.recent.list().await }))
}

#[derive(Debug, Deserialize)]
struct SimulateBody {
    #[serde(default = "default_ip")]
    ip: String,
    #[serde(default = "default_oid")]
    trap_oid: String,
    #[serde(default)]
    alertname: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    varbinds: BTreeMap<String, String>,
    #[serde(default)]
    dry_run: bool,
}

fn default_ip() -> String {
    "10.0.0.1".into()
}
fn default_oid() -> String {
    "1.3.6.1.6.3.1.1.5.3".into()
}

async fn simulate(
    State(st): State<Arc<AppState>>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let trap = simulate_parsed(
        &body.ip,
        &body.trap_oid,
        body.alertname,
        body.severity,
        body.varbinds,
    );
    let policy = st.policies.match_trap(&trap).await;
    let alert = to_ingress_alert_with_policy(&trap, &st.config, policy.as_ref());
    let payload = to_kafka_payload_with_policy(&trap, &st.config, policy.as_ref());
    let mut kafka_ok = false;
    if !body.dry_run {
        if let Some(k) = &st.kafka {
            match k.publish(&trap.peer_ip, payload.clone()).await {
                Ok(()) => {
                    st.stats.kafka_ok.fetch_add(1, Ordering::Relaxed);
                    kafka_ok = true;
                }
                Err(e) => {
                    st.stats.kafka_err.fetch_add(1, Ordering::Relaxed);
                    return Err((
                        StatusCode::BAD_GATEWAY,
                        Json(json!({ "error": format!("kafka: {e:#}"), "alert": alert })),
                    ));
                }
            }
        }
    }
    st.stats.simulated.fetch_add(1, Ordering::Relaxed);
    st.recent
        .push(RecentItem {
            at: Utc::now().to_rfc3339(),
            peer: trap.peer_ip.clone(),
            trap_oid: trap.trap_oid.clone(),
            alertname: alert
                .get("labels")
                .and_then(|l| l.get("alertname"))
                .and_then(|x| x.as_str())
                .unwrap_or(&trap.alertname)
                .to_string(),
            kafka: kafka_ok,
            alert: alert.clone(),
        })
        .await;

    Ok(Json(json!({
        "ok": true,
        "kafka": kafka_ok,
        "topic": st.config.kafka_topic,
        "policy_matched": policy.as_ref().map(|p| json!({
            "id": p.id,
            "name": p.name,
            "match_mode": p.match_mode,
        })),
        "alert": alert,
    })))
}
