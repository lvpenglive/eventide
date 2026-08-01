//! HTTP API for health, stats, simulate, recent (CRUD lives on eventide-server).

use crate::config::TrapConfig;
use crate::kafka_out::KafkaOut;
use crate::normalize::{simulate_parsed, to_ingress_alert_with_policy, to_kafka_payload_with_policy};
use crate::stats::{RecentBuffer, RecentItem, TrapStats};
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use eventide_trap_data::{MibStore, PolicyRedis, PolicyStore};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub config: TrapConfig,
    pub stats: TrapStats,
    pub recent: RecentBuffer,
    pub kafka: Option<KafkaOut>,
    pub mibs: Arc<MibStore>,
    pub policies: Arc<PolicyStore>,
    pub policy_redis: Option<Arc<PolicyRedis>>,
    /// Hot-swappable Trap HTTP token (toml bootstrap + Redis from console).
    pub api_token: Arc<RwLock<String>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/api/stats", get(stats))
        .route("/api/recent", get(recent))
        .route("/api/simulate", post(simulate))
        .route("/api/mibs/reload", post(reload_mibs))
        .route("/api/policies/reload", post(reload_policies))
        .route_layer(from_fn_with_state(state.clone(), require_api_token));

    Router::new()
        .route("/api/health", get(health))
        .merge(protected)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// When `api_token` is set, require `Authorization: Bearer <token>` or `X-Eventide-Trap-Token`.
async fn require_api_token(
    State(st): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let expected = st
        .api_token
        .read()
        .map(|g| g.trim().to_string())
        .unwrap_or_default();
    if expected.is_empty() {
        return next.run(req).await;
    }
    let provided = extract_token(req.headers());
    if provided.as_deref().is_some_and(|p| token_eq(&expected, p)) {
        return next.run(req).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": "unauthorized: missing or invalid Trap api_token (Bearer / X-Eventide-Trap-Token)"
        })),
    )
        .into_response()
}

fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(v) = headers.get("x-eventide-trap-token").and_then(|v| v.to_str().ok()) {
        let t = v.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let rest = auth.strip_prefix("Bearer ").or_else(|| auth.strip_prefix("bearer "))?;
    let t = rest.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn token_eq(expected: &str, provided: &str) -> bool {
    if expected.len() != provided.len() {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(provided.as_bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

async fn health(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "eventide-trap",
        "instance_id": st.config.resolve_instance_id(),
        "listen_udp": st.config.listen_udp,
        "listen_http": st.config.listen_http,
        "ha_vip": st.config.ha_vip,
        "kafka_enabled": st.kafka.is_some(),
        "kafka_topic": st.config.kafka_topic,
        "policy_count": st.policies.list().await.len(),
        "snmpv3_users": st.config.snmpv3_users.len(),
        "auth_required": st
            .api_token
            .read()
            .map(|g| !g.trim().is_empty())
            .unwrap_or(false),
    }))
}

async fn stats(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(st.stats.snapshot())
}

async fn recent(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "items": st.recent.list().await }))
}

async fn reload_mibs(State(st): State<Arc<AppState>>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.reload().await {
        Ok(()) => Ok(Json(json!({
            "ok": true,
            "count": st.mibs.list().await.len(),
        }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("{e:#}") })),
        )),
    }
}

async fn reload_policies(
    State(st): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st
        .policies
        .reload_prefer_redis(st.policy_redis.as_deref())
        .await
    {
        Ok(()) => Ok(Json(json!({
            "ok": true,
            "count": st.policies.list().await.len(),
            "stamp": st.policies.current_stamp().await,
            "source": if st.policy_redis.is_some() { "redis_or_mysql" } else { "mysql" },
        }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("{e:#}") })),
        )),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_eq_rejects_mismatch() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("abc", "ab"));
    }
}
