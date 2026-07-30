//! Trap policy HTTP API on eventide-server.

use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use eventide_trap_data::{ImportMode, TrapPolicy};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/policies", get(list_policies).post(create_policy))
        .route("/api/policies/import", post(import_policies))
        .route("/api/policies/export", get(export_policies))
        .route(
            "/api/policies/{id}",
            get(get_policy).put(update_policy).delete(delete_policy),
        )
}

fn err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": msg.into() })))
}

fn policies_unavailable() -> (StatusCode, Json<Value>) {
    err(StatusCode::SERVICE_UNAVAILABLE, "policy store unavailable")
}

async fn list_policies(
    State(st): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    let items = policies.list().await;
    Ok(Json(json!({
        "items": items,
        "path": policies.backend(),
        "count": items.len(),
    })))
}

async fn get_policy(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    match policies.get(&id).await {
        Some(p) => Ok(Json(json!(p))),
        None => Err(err(StatusCode::NOT_FOUND, "policy not found")),
    }
}

async fn create_policy(
    State(st): State<Arc<AppState>>,
    Json(mut body): Json<TrapPolicy>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    body.id.clear();
    match policies.upsert(body).await {
        Ok(p) => {
            st.sync_policies_to_redis().await;
            Ok(Json(json!({ "ok": true, "item": p })))
        }
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn update_policy(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut body): Json<TrapPolicy>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    body.id = id;
    match policies.upsert(body).await {
        Ok(p) => {
            st.sync_policies_to_redis().await;
            Ok(Json(json!({ "ok": true, "item": p })))
        }
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn delete_policy(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    match policies.delete(&id).await {
        Ok(()) => {
            st.sync_policies_to_redis().await;
            Ok(Json(json!({ "ok": true })))
        }
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

#[derive(Debug, Deserialize)]
struct ImportQuery {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    replace: bool,
}

fn import_mode_from_query(mode: Option<&str>, replace: bool) -> ImportMode {
    if let Some(m) = mode.filter(|s| !s.trim().is_empty()) {
        return ImportMode::parse(m);
    }
    if replace {
        ImportMode::Replace
    } else {
        ImportMode::Merge
    }
}

async fn import_policies(
    State(st): State<Arc<AppState>>,
    Query(q): Query<ImportQuery>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    if body.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "empty body"));
    }
    let mode = import_mode_from_query(q.mode.as_deref(), q.replace);
    let result = if looks_like_zip_xlsx(&body) {
        policies.import_xlsx(&body, mode).await
    } else if body.first() == Some(&b'{') || body.first() == Some(&b'[') {
        policies.import_json(&body, mode).await
    } else {
        policies.import_xlsx(&body, mode).await
    };
    match result {
        Ok(r) => {
            st.sync_policies_to_redis().await;
            Ok(Json(json!({ "ok": true, "result": r })))
        }
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

fn looks_like_zip_xlsx(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x50 && bytes[1] == 0x4B
}

async fn export_policies(
    State(st): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let Some(policies) = st.policies.as_ref() else {
        return Err(policies_unavailable());
    };
    match policies.export_xlsx().await {
        Ok(body) => Ok(xlsx_response("trap-policies-active.xlsx", body)),
        Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
    }
}

pub fn xlsx_response(filename: &str, body: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, XLSX_MIME)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
