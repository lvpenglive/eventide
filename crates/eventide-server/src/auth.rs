//! JWT login / auth middleware + route permission checks.

use crate::iam::{has_perm, route_permission};
use crate::password::{
    hash_password, password_age_status, validate_password_complexity, verify_password,
};
use crate::state::AppState;
use axum::extract::ConnectInfo;
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
use std::net::SocketAddr;
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
    pub password_status: serde_json::Value,
}

fn auth_password_status(
    state: &AppState,
    changed_at: chrono::DateTime<Utc>,
) -> serde_json::Value {
    let auth = &state.config.auth;
    password_age_status(
        changed_at,
        auth.password_max_age_days,
        auth.password_warn_days,
    )
    .to_json()
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<serde_json::Value>)> {
    let ip = addr.ip().to_string();
    let username = req.username.trim().to_string();

    if let Err(secs) = state.login_limiter.check(&ip, &username) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": format!("登录失败次数过多，请 {secs} 秒后再试"),
                "retry_after_seconds": secs,
            })),
        ));
    }

    let user = state.db.get_user_by_username(&username).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let Some(user) = user else {
        state.login_limiter.record_failure(&ip, &username);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    };
    if !user.enabled {
        state.login_limiter.record_failure(&ip, &username);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "账号已禁用" })),
        ));
    }
    if !verify_password(&req.password, &user.password_hash) {
        state.login_limiter.record_failure(&ip, &username);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "用户名或密码错误" })),
        ));
    }

    state.login_limiter.record_success(&ip, &username);

    let mut perms = state
        .db
        .permissions_for_roles(&user.role_ids)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    // Fallback: admin user gets wildcard when IAM data is uninitialized
    if perms.is_empty() && username == "admin" {
        tracing::warn!("admin user has no permissions in DB, granting wildcard '*' as fallback");
        perms = vec!["*".into()];
    }

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

    let password_status = auth_password_status(&state, user.password_changed_at);

    Ok(Json(LoginResponse {
        token,
        username: user.username,
        expires_at: exp.to_rfc3339(),
        permissions: perms,
        password_status,
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
        "password_max_age_days": state.config.auth.password_max_age_days,
        "password_warn_days": state.config.auth.password_warn_days,
    });

    if let Ok(uid) = uuid::Uuid::parse_str(&claims.uid) {
        if let Ok(Some(u)) = state.db.get_user(uid) {
            body["display_name"] = serde_json::json!(u.display_name);
            body["department_id"] = serde_json::json!(u.department_id);
            body["role_ids"] = serde_json::json!(u.role_ids);
            body["enabled"] = serde_json::json!(u.enabled);
            body["password_status"] = auth_password_status(&state, u.password_changed_at);
            if let Ok(mut perms) = state.db.permissions_for_roles(&u.role_ids) {
                // Fallback: admin user gets wildcard when IAM data is uninitialized
                if perms.is_empty() && u.username == "admin" {
                    perms = vec!["*".into()];
                }
                body["permissions"] = serde_json::json!(perms);
            }
        }
    }

    Ok(Json(body))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Authenticated user changes their own password.
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    AuthUser(claims): AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = uuid::Uuid::parse_str(&claims.uid).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "无效的用户" })),
        )
    })?;
    let mut user = state
        .db
        .get_user(uid)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "用户不存在" })),
        ))?;

    if !verify_password(&req.current_password, &user.password_hash) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "当前密码不正确" })),
        ));
    }
    let new_pw = req.new_password.trim();
    validate_password_complexity(new_pw).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
    })?;
    if verify_password(new_pw, &user.password_hash) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "新密码不能与当前密码相同" })),
        ));
    }

    let now = Utc::now();
    user.password_hash = hash_password(new_pw);
    user.password_changed_at = now;
    user.updated_at = now;
    state.db.upsert_user(&user).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "password_status": auth_password_status(&state, user.password_changed_at),
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
        // Self password change is also allowed (account hygiene).
        let allowed =
            path == "/api/license" || path == "/api/auth/change-password";
        if !allowed && !state.license.is_writable() {
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
