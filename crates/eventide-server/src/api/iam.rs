//! Users / roles / departments REST API.

use crate::auth::AuthUser;
use crate::iam::{catalog_json, Department, Role, UserAccount};
use crate::password::{hash_password, validate_password_complexity};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

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
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    #[allow(dead_code)]
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

fn parse_id(id: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::bad("invalid uuid"))
}

fn would_lose_last_admin(
    state: &AppState,
    target_id: Uuid,
    next_enabled: bool,
    next_role_ids: &[Uuid],
) -> ApiResult<bool> {
    let before = state.db.count_star_admins().map_err(ApiError::internal)?;
    let target = state
        .db
        .get_user(target_id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    let had_star = {
        let p = state
            .db
            .permissions_for_roles(&target.role_ids)
            .map_err(ApiError::internal)?;
        crate::iam::has_perm(&p, "*") && target.enabled
    };
    let will_have = if !next_enabled {
        false
    } else {
        let p = state
            .db
            .permissions_for_roles(next_role_ids)
            .map_err(ApiError::internal)?;
        crate::iam::has_perm(&p, "*")
    };
    Ok(had_star && !will_have && before <= 1)
}

// ---------- permissions catalog ----------

pub async fn list_permissions() -> impl IntoResponse {
    Json(catalog_json())
}

// ---------- departments ----------

#[derive(Deserialize)]
pub struct DepartmentInput {
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub async fn list_departments(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Department>>> {
    Ok(Json(state.db.list_departments().map_err(ApiError::internal)?))
}

pub async fn create_department(
    State(state): State<Arc<AppState>>,
    Json(input): Json<DepartmentInput>,
) -> ApiResult<Json<Department>> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("部门名称不能为空"));
    }
    if let Some(pid) = input.parent_id {
        if state
            .db
            .get_department(pid)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("上级部门不存在"));
        }
    }
    let now = Utc::now();
    let d = Department {
        id: Uuid::new_v4(),
        name,
        parent_id: input.parent_id,
        sort_order: input.sort_order,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    };
    state.db.upsert_department(&d).map_err(ApiError::internal)?;
    Ok(Json(d))
}

pub async fn update_department(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<DepartmentInput>,
) -> ApiResult<Json<Department>> {
    let id = parse_id(&id)?;
    let mut d = state
        .db
        .get_department(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("部门不存在"))?;
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("部门名称不能为空"));
    }
    if let Some(pid) = input.parent_id {
        if pid == id {
            return Err(ApiError::bad("不能将部门设为自身的上级"));
        }
        if state
            .db
            .get_department(pid)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("上级部门不存在"));
        }
    }
    d.name = name;
    d.parent_id = input.parent_id;
    d.sort_order = input.sort_order;
    d.enabled = input.enabled;
    d.updated_at = Utc::now();
    state.db.upsert_department(&d).map_err(ApiError::internal)?;
    Ok(Json(d))
}

pub async fn delete_department(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    match state.db.delete_department(id) {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(ApiError::not_found("部门不存在")),
        Err(e) => Err(ApiError::bad(e.to_string())),
    }
}

// ---------- roles ----------

#[derive(Deserialize)]
pub struct RoleInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub permissions: Option<Vec<String>>,
}

pub async fn list_roles(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Role>>> {
    Ok(Json(state.db.list_roles().map_err(ApiError::internal)?))
}

pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RoleInput>,
) -> ApiResult<Json<Role>> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("角色名称不能为空"));
    }
    let now = Utc::now();
    let r = Role {
        id: Uuid::new_v4(),
        name,
        description: input.description,
        permissions: input.permissions.unwrap_or_default(),
        is_system: false,
        created_at: now,
        updated_at: now,
    };
    state.db.upsert_role(&r).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            ApiError::bad("角色名称已存在")
        } else {
            ApiError::internal(e)
        }
    })?;
    Ok(Json(r))
}

pub async fn update_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<RoleInput>,
) -> ApiResult<Json<Role>> {
    let id = parse_id(&id)?;
    let mut r = state
        .db
        .get_role(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("角色不存在"))?;
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad("角色名称不能为空"));
    }
    // System role permissions cannot be modified
    if r.is_system {
        if let Some(ref perms) = input.permissions {
            if perms != &r.permissions {
                return Err(ApiError::bad("系统角色的权限不可修改"));
            }
        }
    } else if let Some(perms) = input.permissions {
        r.permissions = perms;
    }
    r.name = name;
    r.description = input.description;
    r.updated_at = Utc::now();
    state.db.upsert_role(&r).map_err(ApiError::internal)?;
    Ok(Json(r))
}

pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let role = state
        .db
        .get_role(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("角色不存在"))?;
    if role.is_system {
        return Err(ApiError::bad("系统角色不可删除"));
    }
    match state.db.delete_role(id) {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(ApiError::not_found("角色不存在")),
        Err(e) => Err(ApiError::bad(e.to_string())),
    }
}

// ---------- users ----------

#[derive(Deserialize)]
pub struct UserInput {
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    pub password: Option<String>,
    pub department_id: Option<Uuid>,
    pub role_ids: Option<Vec<Uuid>>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ResetPasswordInput {
    pub password: String,
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let rows = state.db.list_users().map_err(ApiError::internal)?;
    Ok(Json(rows.iter().map(|u| u.public_json()).collect()))
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let username = input.username.trim().to_string();
    if username.is_empty() {
        return Err(ApiError::bad("用户名不能为空"));
    }
    let password = input
        .password
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad("请设置初始密码"))?;
    validate_password_complexity(password).map_err(ApiError::bad)?;
    if state
        .db
        .get_user_by_username(&username)
        .map_err(ApiError::internal)?
        .is_some()
    {
        return Err(ApiError::bad("用户名已存在"));
    }
    if let Some(did) = input.department_id {
        if state
            .db
            .get_department(did)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("部门不存在"));
        }
    }
    for rid in input.role_ids.as_deref().unwrap_or(&[]) {
        if state
            .db
            .get_role(*rid)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("角色不存在"));
        }
    }
    let now = Utc::now();
    let u = UserAccount {
        id: Uuid::new_v4(),
        username,
        display_name: input.display_name,
        password_hash: hash_password(password),
        department_id: input.department_id,
        role_ids: input.role_ids.unwrap_or_default(),
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
        password_changed_at: now,
    };
    state.db.upsert_user(&u).map_err(ApiError::internal)?;
    Ok(Json(u.public_json()))
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let id = parse_id(&id)?;
    let mut u = state
        .db
        .get_user(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;

    let username = input.username.trim().to_string();
    if username.is_empty() {
        return Err(ApiError::bad("用户名不能为空"));
    }
    if let Some(other) = state
        .db
        .get_user_by_username(&username)
        .map_err(ApiError::internal)?
    {
        if other.id != id {
            return Err(ApiError::bad("用户名已存在"));
        }
    }

    // cannot disable self
    if claims.uid == id.to_string() && !input.enabled {
        return Err(ApiError::bad("不能禁用当前登录账号"));
    }

    // admin user must keep at least one role
    if u.username == "admin" {
        if let Some(roles) = &input.role_ids {
            if roles.is_empty() {
                return Err(ApiError::bad("admin 用户至少需要保留一个角色"));
            }
        }
    }

    if would_lose_last_admin(&state, id, input.enabled, input.role_ids.as_deref().unwrap_or(&[]))? {
        return Err(ApiError::bad("不能移除最后一个拥有全部权限的管理员"));
    }

    if let Some(did) = input.department_id {
        if state
            .db
            .get_department(did)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("部门不存在"));
        }
    }
    for rid in input.role_ids.as_deref().unwrap_or(&[]) {
        if state
            .db
            .get_role(*rid)
            .map_err(ApiError::internal)?
            .is_none()
        {
            return Err(ApiError::bad("角色不存在"));
        }
    }

    u.username = username;
    u.display_name = input.display_name;
    u.department_id = input.department_id;
    if let Some(roles) = input.role_ids {
        u.role_ids = roles;
    }
    u.enabled = input.enabled;
    if let Some(pw) = input.password.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        validate_password_complexity(pw).map_err(ApiError::bad)?;
        u.password_hash = hash_password(pw);
        u.password_changed_at = Utc::now();
    }
    u.updated_at = Utc::now();
    state.db.upsert_user(&u).map_err(ApiError::internal)?;
    Ok(Json(u.public_json()))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AuthUser(claims): AuthUser,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    if claims.uid == id.to_string() {
        return Err(ApiError::bad("不能删除当前登录账号"));
    }
    let user = state
        .db
        .get_user(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    if would_lose_last_admin(&state, id, false, &user.role_ids)? {
        return Err(ApiError::bad("不能删除最后一个拥有全部权限的管理员"));
    }
    if state.db.delete_user(id).map_err(ApiError::internal)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("用户不存在"))
    }
}

pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<ResetPasswordInput>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let pw = input.password.trim();
    validate_password_complexity(pw).map_err(ApiError::bad)?;
    let mut u = state
        .db
        .get_user(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    u.password_hash = hash_password(pw);
    u.password_changed_at = Utc::now();
    u.updated_at = Utc::now();
    state.db.upsert_user(&u).map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}
