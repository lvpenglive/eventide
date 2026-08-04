//! JWT login / auth middleware + route permission checks.

use crate::iam::{has_perm, route_permission};
use crate::password::verify_password;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::extract::State;
use axum::http::request::Parts;
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
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub perms: Vec<String>,
}

impl Claims {
    pub fn has_perm(&self, need: &str) -> bool {
        has_perm(&self.perms, need)
    }
}

/// Extractor for authenticated user claims (inserted by require_auth).
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "未登录" })),
            ))
    }
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
    pub permissions: Vec<String>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user = state
        .db
        .get_user_by_username(req.username.trim())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let Some(user) = user else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    };
    if !user.enabled {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "账号已禁用" })),
        ));
    }
    if !verify_password(&req.password, &user.password_hash) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    }

    let perms = state
        .db
        .permissions_for_roles(&user.role_ids)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let auth = &state.config.auth;
    let now = Utc::now();
    let exp = now + Duration::hours(auth.token_ttl_hours as i64);
    let claims = Claims {
        sub: user.username.clone(),
        uid: user.id.to_string(),
        perms: perms.clone(),
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
        username: user.username,
        expires_at: exp.to_rfc3339(),
        permissions: perms,
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut body = serde_json::json!({
        "username": claims.sub,
        "uid": claims.uid,
        "permissions": claims.perms,
        "listen": state.config.listen,
        "scheduler_tick_seconds": state.config.scheduler_tick_seconds,
        "mysql_url": state.config.mysql_url,
        "redis_url": state.config.redis_url,
        "static_dir": state.config.static_dir,
        "token_ttl_hours": state.config.auth.token_ttl_hours,
    });

    if let Ok(uid) = uuid::Uuid::parse_str(&claims.uid) {
        if let Ok(Some(u)) = state.db.get_user(uid) {
            body["display_name"] = serde_json::json!(u.display_name);
            body["department_id"] = serde_json::json!(u.department_id);
            body["role_ids"] = serde_json::json!(u.role_ids);
            body["enabled"] = serde_json::json!(u.enabled);
            if let Ok(perms) = state.db.permissions_for_roles(&u.role_ids) {
                body["permissions"] = serde_json::json!(perms);
            }
        }
    }

    Ok(Json(body))
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

/// After require_auth: block configuration writes when license/trial is not writable.
pub async fn require_writable_license(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_uppercase();
    let write = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    if write {
        let path = req.uri().path();
        // Allow importing / clearing license while in read-only grace.
        let license_mgmt = path == "/api/license";
        if !license_mgmt && !state.license.is_writable() {
            return (
                StatusCode::PAYMENT_REQUIRED,
                Json(serde_json::json!({
                    "error": "许可证无效或已过期，当前为只读宽限",
                    "code": "license_readonly",
                })),
            )
                .into_response();
        }
    }
    next.run(req).await
}

/// After require_auth: enforce RBAC based on method + path.
pub async fn require_route_perm(req: Request<axum::body::Body>, next: Next) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    if let Some(need) = route_permission(&method, &path) {
        let allowed = req
            .extensions()
            .get::<Claims>()
            .map(|c| c.has_perm(need))
            .unwrap_or(false);
        if !allowed {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": format!("无权限：{need}"),
                    "permission": need,
                })),
            )
                .into_response();
        }
    }
    next.run(req).await
}
