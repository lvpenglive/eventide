//! MIB library HTTP handlers.

use crate::http::AppState;
use crate::policy_store::ImportMode;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/mibs", get(list_mibs).post(upload_mib))
        .route("/api/mibs/export-policies", get(export_all_policies))
        .route("/api/mibs/apply-policies", post(apply_all_policies))
        .route("/api/mibs/reload", post(reload))
        .route("/api/mibs/{id}", get(get_mib).delete(delete_mib))
        .route("/api/mibs/{id}/children", get(children))
        .route("/api/mibs/{id}/node", get(node_detail))
        .route("/api/mibs/{id}/notifications", get(notifications))
        .route("/api/mibs/{id}/export-policies", get(export_module_policies))
        .route("/api/mibs/{id}/apply-policies", post(apply_module_policies))
}

fn err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "error": msg.into() })),
    )
}

async fn list_mibs(State(st): State<Arc<AppState>>) -> Json<Value> {
    let items = st.mibs.list().await;
    let load_error = st.mibs.load_error().await;
    Json(json!({
        "items": items,
        "mib_dir": st.mibs.dir().display().to_string(),
        "backend": st.mibs.backend(),
        "load_error": load_error,
    }))
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    filename: String,
}

async fn upload_mib(
    State(st): State<Arc<AppState>>,
    Query(q): Query<UploadQuery>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if body.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "empty body"));
    }
    if body.len() > 16 * 1024 * 1024 {
        return Err(err(StatusCode::PAYLOAD_TOO_LARGE, "MIB 文件不能超过 16MB"));
    }
    match st.mibs.upload(&q.filename, &body).await {
        Ok(entry) => Ok(Json(json!({ "ok": true, "item": entry }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn get_mib(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.get(&id).await {
        Some(e) => Ok(Json(json!(e))),
        None => Err(err(StatusCode::NOT_FOUND, "module not found")),
    }
}

async fn delete_mib(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.delete(&id).await {
        Ok(()) => Ok(Json(json!({ "ok": true }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

#[derive(Debug, Deserialize)]
struct ChildrenQuery {
    #[serde(default)]
    oid: String,
}

async fn children(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ChildrenQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let parent = if q.oid.is_empty() {
        None
    } else {
        Some(q.oid.as_str())
    };
    match st.mibs.children(&id, parent).await {
        Ok(nodes) => Ok(Json(json!({ "module": id, "parent_oid": q.oid, "children": nodes }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

#[derive(Debug, Deserialize)]
struct NodeQuery {
    oid: String,
}

async fn node_detail(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<NodeQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.node_detail(&id, &q.oid).await {
        Ok(n) => Ok(Json(json!(n))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn notifications(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.notifications(&id).await {
        Ok(items) => Ok(Json(json!({ "module": id, "items": items }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn export_module_policies(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    match st.mibs.export_policies(Some(&id)).await {
        Ok(export) => match crate::policy_xlsx::write_xlsx_from_drafts(&export.policies) {
            Ok(body) => Ok(crate::policy_api::xlsx_response(
                &format!("trap-policies-{id}.xlsx"),
                body,
            )),
            Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
        },
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn export_all_policies(
    State(st): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    match st.mibs.export_policies(None).await {
        Ok(export) => match crate::policy_xlsx::write_xlsx_from_drafts(&export.policies) {
            Ok(body) => Ok(crate::policy_api::xlsx_response("trap-policies-all.xlsx", body)),
            Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
        },
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

#[derive(Debug, Deserialize)]
struct ApplyQuery {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    replace: bool,
}

fn apply_mode(q: &ApplyQuery) -> ImportMode {
    if let Some(m) = q.mode.as_deref().filter(|s| !s.trim().is_empty()) {
        ImportMode::parse(m)
    } else if q.replace {
        ImportMode::Replace
    } else {
        ImportMode::Merge
    }
}

async fn apply_module_policies(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ApplyQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let export = st
        .mibs
        .export_policies(Some(&id))
        .await
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    match st.policies.import_drafts(export.policies, apply_mode(&q)).await {
        Ok(r) => Ok(Json(json!({ "ok": true, "result": r }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn apply_all_policies(
    State(st): State<Arc<AppState>>,
    Query(q): Query<ApplyQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let export = st
        .mibs
        .export_policies(None)
        .await
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    match st.policies.import_drafts(export.policies, apply_mode(&q)).await {
        Ok(r) => Ok(Json(json!({ "ok": true, "result": r }))),
        Err(e) => Err(err(StatusCode::BAD_REQUEST, format!("{e:#}"))),
    }
}

async fn reload(State(st): State<Arc<AppState>>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match st.mibs.reload().await {
        Ok(()) => Ok(Json(json!({
            "ok": true,
            "items": st.mibs.list().await,
            "load_error": st.mibs.load_error().await,
        }))),
        Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
    }
}
