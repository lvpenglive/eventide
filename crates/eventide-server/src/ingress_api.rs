//! Ingress webhook handlers (Alertmanager + generic).

use crate::notify_pipeline::{persist_and_notify, synthetic_rule_for_ingress};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use eventide_core::{
    apply_ingress, parse_alertmanager, parse_ingress_payload_with_options, AlertmanagerWebhook,
    IngressKind, IngressRoute,
};
use std::sync::Arc;
use uuid::Uuid;

pub async fn receive_alertmanager(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<AlertmanagerWebhook>,
) -> impl IntoResponse {
    match process_ingress(&state, &id, &headers, IngressKind::Alertmanager, |route| {
        let alerts = parse_alertmanager(&body);
        Ok((route, alerts))
    })
    .await
    {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "accepted": n }))).into_response(),
        Err(resp) => resp,
    }
}

pub async fn receive_generic(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    match process_ingress(&state, &id, &headers, IngressKind::Generic, |route| {
        let alerts = parse_ingress_payload_with_options(&body, &route.options)?;
        Ok((route, alerts))
    })
    .await
    {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "accepted": n }))).into_response(),
        Err(resp) => resp,
    }
}

/// Auto-dispatch by route.kind.
pub async fn receive_auto(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let Some(_permit) = state.pressure.try_enter() else {
        return too_many_requests();
    };
    let id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid uuid" })),
            )
                .into_response();
        }
    };
    let route = match state.db.get_ingress_route(id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "ingress route not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };
    if let Err(resp) = check_auth(&route, &headers) {
        return resp;
    }
    if !route.enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "ingress disabled" })),
        )
            .into_response();
    }

    let result = match route.kind {
        IngressKind::Alertmanager => {
            let parsed: Result<AlertmanagerWebhook, _> = serde_json::from_slice(&body);
            match parsed {
                Ok(w) => ingest_list(&state, &route, parse_alertmanager(&w)).await,
                Err(e) => Err(e.to_string()),
            }
        }
        IngressKind::Generic => match parse_ingress_payload_with_options(&body, &route.options) {
            Ok(alerts) => ingest_list(&state, &route, alerts).await,
            Err(e) => Err(e),
        },
        IngressKind::Kafka => match parse_ingress_payload_with_options(&body, &route.options) {
            // Allow manual push test against a Kafka-configured route.
            Ok(alerts) => ingest_list(&state, &route, alerts).await,
            Err(e) => Err(e),
        },
    };

    match result {
        Ok(n) => (StatusCode::OK, Json(serde_json::json!({ "accepted": n }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

async fn process_ingress<F>(
    state: &Arc<AppState>,
    id: &str,
    headers: &HeaderMap,
    expected_kind: IngressKind,
    build: F,
) -> Result<usize, axum::response::Response>
where
    F: FnOnce(IngressRoute) -> Result<(IngressRoute, Vec<eventide_core::IngressAlert>), String>,
{
    let Some(_permit) = state.pressure.try_enter() else {
        return Err(too_many_requests());
    };
    let id = Uuid::parse_str(id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid uuid" })),
        )
            .into_response()
    })?;
    let route = state
        .db
        .get_ingress_route(id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "ingress route not found" })),
            )
                .into_response()
        })?;

    if route.kind != expected_kind {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "route kind is {}, use matching endpoint or /api/ingress/{{id}}/push",
                    route.kind.as_str()
                )
            })),
        )
            .into_response());
    }
    check_auth(&route, headers)?;
    if !route.enabled {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "ingress disabled" })),
        )
            .into_response());
    }

    let (route, alerts) = build(route).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()
    })?;

    ingest_list(state, &route, alerts).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()
    })
}

fn too_many_requests() -> axum::response::Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({
            "error": "ingress overloaded",
            "hint": "storm.ingress_max_inflight limit reached; retry later"
        })),
    )
        .into_response()
}

fn check_auth(route: &IngressRoute, headers: &HeaderMap) -> Result<(), axum::response::Response> {
    // HTTP ingress always requires a configured token (legacy empty-token routes are rejected).
    let Some(expected) = route.token.as_ref().filter(|t| !t.trim().is_empty()) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "ingress token required",
                "hint": "configure a non-empty Token on this HTTP route in the console"
            })),
        )
            .into_response());
    };
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.to_string()))
        .or_else(|| {
            headers
                .get("x-eventide-token")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });
    if provided.as_deref() == Some(expected.as_str()) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid token" })),
        )
            .into_response())
    }
}

pub(crate) async fn ingest_list(
    state: &Arc<AppState>,
    route: &IngressRoute,
    alerts: Vec<eventide_core::IngressAlert>,
) -> Result<usize, String> {
    let now = Utc::now();
    let mut n = 0;
    for incoming in alerts {
        let existing = state
            .db
            .get_alert_by_fingerprint(&{
                // apply_ingress prefixes fingerprint — peek by computing same way
                let fp = incoming
                    .fingerprint
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| {
                        eventide_core::alert_fingerprint(route.id, &incoming.labels)
                    });
                format!("{}:{}", route.id, fp)
            })
            .map_err(|e| e.to_string())?;
        let had = existing.is_some();
        let (event, transition) = apply_ingress(route, &incoming, existing, now);
        let rule = synthetic_rule_for_ingress(route, &event);
        persist_and_notify(
            state,
            &rule,
            &route.channel_ids,
            event,
            transition,
            had,
            now,
        )
        .await
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}
