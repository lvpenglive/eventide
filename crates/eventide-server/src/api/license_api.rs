//! Product license HTTP API.

use crate::auth::AuthUser;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use eventide_license::{LicenseFile, LicenseRequest};
use serde::Deserialize;
use std::sync::Arc;

type ApiResult<T> = Result<T, ApiError>;

pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: msg.into(),
        }
    }
    fn internal(e: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

#[derive(Deserialize)]
pub struct LicenseRequestQuery {
    /// Optional customer / organization name written into the request file.
    #[serde(default)]
    pub customer: Option<String>,
}

#[derive(Deserialize)]
pub struct LicenseImport {
    /// Raw JWT, or full `.eventide-lic.json` text.
    #[serde(default)]
    pub token: Option<String>,
    /// Alias: paste entire license file JSON as string.
    #[serde(default)]
    pub content: Option<String>,
}

pub async fn get_license(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<crate::license::LicenseSnapshot>> {
    let snap = state
        .license
        .refresh(&state.db)
        .map_err(ApiError::internal)?;
    Ok(Json(snap))
}

/// Export authorization request file for the vendor to sign.
pub async fn get_license_request(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<LicenseRequestQuery>,
) -> ApiResult<Json<LicenseRequest>> {
    let snap = state
        .license
        .refresh(&state.db)
        .map_err(ApiError::internal)?;
    let req = LicenseRequest::new(snap.install_id, q.customer);
    Ok(Json(req))
}

pub async fn put_license(
    State(state): State<Arc<AppState>>,
    AuthUser(claims): AuthUser,
    Json(body): Json<LicenseImport>,
) -> ApiResult<Json<crate::license::LicenseSnapshot>> {
    if !claims.has_perm("settings:write") {
        return Err(ApiError::forbidden("需要 settings:write"));
    }
    let raw = body
        .content
        .as_deref()
        .or(body.token.as_deref())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return Err(ApiError::bad("请粘贴授权文件内容或选择授权文件"));
    }
    let file = LicenseFile::parse_import(raw).map_err(|e| ApiError::bad(e.to_string()))?;
    let snap = state
        .license
        .import(&state.db, file.token.trim())
        .map_err(|e| ApiError::bad(e.to_string()))?;
    Ok(Json(snap))
}

pub async fn delete_license(
    State(state): State<Arc<AppState>>,
    AuthUser(claims): AuthUser,
) -> ApiResult<Json<crate::license::LicenseSnapshot>> {
    if !claims.has_perm("settings:write") {
        return Err(ApiError::forbidden("需要 settings:write"));
    }
    let snap = state
        .license
        .clear_commercial(&state.db)
        .map_err(ApiError::internal)?;
    Ok(Json(snap))
}
