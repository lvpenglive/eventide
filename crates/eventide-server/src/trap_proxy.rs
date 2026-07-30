//! Reverse-proxy `/trap-api/*` → Trap service HTTP API.

use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

fn trap_base(state: &AppState) -> Option<String> {
    let u = state.config.trap.api_url.trim();
    if u.is_empty() {
        None
    } else {
        Some(u.trim_end_matches('/').to_string())
    }
}

pub async fn forward_root(State(state): State<Arc<AppState>>, req: Request) -> Response {
    forward_inner(state, String::new(), req).await
}

pub async fn forward(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    req: Request,
) -> Response {
    forward_inner(state, path, req).await
}

async fn forward_inner(state: Arc<AppState>, path: String, req: Request) -> Response {
    let Some(base) = trap_base(&state) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "Trap 服务未配置：请在 eventide.toml 设置 [trap] api_url"
            })),
        )
            .into_response();
    };

    let suffix = path.trim_start_matches('/').to_string();
    let qs = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let url = if suffix.is_empty() {
        format!("{base}/api/health{qs}")
    } else {
        format!("{base}/{suffix}{qs}")
    };

    let method = req.method().clone();
    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": format!("read body: {e}") })),
            )
                .into_response();
        }
    };

    let client = reqwest::Client::new();
    let mut builder = client.request(method, &url);
    for (k, v) in headers.iter() {
        let name = k.as_str();
        if matches!(
            name,
            "host" | "connection" | "content-length" | "transfer-encoding" | "authorization"
        ) {
            continue;
        }
        if let Ok(v) = v.to_str() {
            builder = builder.header(name, v);
        }
    }

    let upstream = match builder.body(body_bytes).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({
                    "error": format!("Trap 服务不可达 ({url}): {e}")
                })),
            )
                .into_response();
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let ct = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({ "error": format!("upstream body: {e}") })),
            )
                .into_response();
        }
    };

    let mut res = Response::builder().status(status);
    if let Some(ct) = ct {
        res = res.header(header::CONTENT_TYPE, ct);
    }
    res.body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
