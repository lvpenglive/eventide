//! JWT login / auth middleware.

use crate::state::AppState;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub username: String,
    pub expires_at: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<serde_json::Value>)> {
    let auth = &state.config.auth;
    if req.username != auth.username || req.password != auth.password {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    }

    let now = Utc::now();
    let exp = now + Duration::hours(auth.token_ttl_hours as i64);
    let claims = Claims {
        sub: auth.username.clone(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(auth.jwt_secret.as_bytes()),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(LoginResponse {
        token,
        username: auth.username.clone(),
        expires_at: exp.to_rfc3339(),
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(Json(serde_json::json!({
        "username": claims.sub,
        "listen": state.config.listen,
        "scheduler_tick_seconds": state.config.scheduler_tick_seconds,
        "database_path": state.config.database_path,
        "token_ttl_hours": state.config.auth.token_ttl_hours,
    })))
}

pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "未登录" })),
        )
            .into_response();
    };

    let mut validation = Validation::default();
    validation.validate_exp = true;
    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(state.config.auth.jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => {
            req.extensions_mut().insert(data.claims);
            next.run(req).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "登录已失效，请重新登录" })),
        )
            .into_response(),
    }
}
