//! Settings API: alert history + Trap HTTP api_token + storm.

use crate::alert_history::ALERT_HISTORY_KEY;
use crate::config::StormConfig;
use crate::state::{AppState, STORM_CONFIG_KEY};
use crate::trap_token::{
    generate_token, mask_token, TrapApiTokenPrefs, TRAP_API_TOKEN_KEY,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct AlertHistorySettingsView {
    pub write_to_es: bool,
    pub search_store: String,
    pub es_configured: bool,
    pub es_url: String,
    pub es_index: String,
    pub es_username: String,
    /// Never returns the real password; true if one is stored.
    pub es_password_set: bool,
}

#[derive(Debug, Deserialize)]
pub struct AlertHistorySettingsUpdate {
    pub write_to_es: Option<bool>,
    pub search_store: Option<String>,
    pub es_url: Option<String>,
    pub es_index: Option<String>,
    pub es_username: Option<String>,
    /// Empty / omitted = keep existing password; non-empty = replace.
    pub es_password: Option<String>,
}

pub async fn get_alert_history(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<AlertHistorySettingsView>> {
    Ok(Json(view(&state)))
}

pub async fn put_alert_history(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AlertHistorySettingsUpdate>,
) -> ApiResult<Json<AlertHistorySettingsView>> {
    let mut prefs = state.alert_history_prefs();
    if let Some(w) = body.write_to_es {
        prefs.write_to_es = w;
    }
    if let Some(s) = body.search_store {
        prefs.search_store = s;
    }
    if let Some(u) = body.es_url {
        prefs.es_url = u;
    }
    if let Some(i) = body.es_index {
        prefs.es_index = i;
    }
    if let Some(u) = body.es_username {
        prefs.es_username = u;
    }
    if let Some(p) = body.es_password {
        if !p.is_empty() {
            prefs.es_password = p;
        }
    }
    prefs = prefs.normalize();

    if prefs.write_to_es && !prefs.es_configured() {
        return Err(ApiError::bad("请先填写 Elasticsearch 地址"));
    }
    if prefs.search_store == "es" && !prefs.es_configured() {
        return Err(ApiError::bad("请先填写 Elasticsearch 地址，才能将默认列表设为历史事件"));
    }

    // Rebuild client before persisting so bad URL fails the save.
    let client = prefs.build_es_client().map_err(|e| {
        ApiError::bad(format!("无法连接 Elasticsearch 配置: {e}"))
    })?;
    // Optional lightweight ping could go here; for now accept build success.

    let json = serde_json::to_string(&prefs).map_err(ApiError::internal)?;
    state
        .db
        .set_kv(ALERT_HISTORY_KEY, &json)
        .map_err(ApiError::internal)?;
    state.set_alert_history_prefs(prefs);
    state.set_es_client(client);

    Ok(Json(view(&state)))
}

fn view(state: &AppState) -> AlertHistorySettingsView {
    let prefs = state.alert_history_prefs();
    AlertHistorySettingsView {
        write_to_es: prefs.write_to_es,
        search_store: prefs.search_store.clone(),
        es_configured: prefs.es_configured() && state.es_client().is_some(),
        es_url: prefs.es_url.clone(),
        es_index: prefs.es_index.clone(),
        es_username: prefs.es_username.clone(),
        es_password_set: !prefs.es_password.is_empty(),
    }
}

#[derive(Debug, Serialize)]
pub struct TrapTokenView {
    pub configured: bool,
    pub token_preview: String,
    pub token_length: usize,
    /// `runtime` | `toml` | `empty`
    pub source: String,
    /// Only set after rotate/save in the same response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub redis_synced: bool,
}

#[derive(Debug, Deserialize)]
pub struct TrapTokenUpdate {
    /// Set explicit token (trimmed). Ignored when `rotate` / `clear`.
    pub token: Option<String>,
    /// Generate a new random token.
    #[serde(default)]
    pub rotate: bool,
    /// Clear runtime token (falls back to toml; empty toml = open HTTP).
    #[serde(default)]
    pub clear: bool,
}

pub async fn get_trap_token(State(state): State<Arc<AppState>>) -> ApiResult<Json<TrapTokenView>> {
    Ok(Json(trap_token_view(&state, None)))
}

pub async fn put_trap_token(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TrapTokenUpdate>,
) -> ApiResult<Json<TrapTokenView>> {
    let new_token = if body.clear {
        String::new()
    } else if body.rotate {
        generate_token()
    } else if let Some(t) = body.token {
        let t = t.trim().to_string();
        if t.len() < 8 {
            return Err(ApiError::bad("token 至少 8 个字符"));
        }
        t
    } else {
        return Err(ApiError::bad("请提供 token、rotate=true 或 clear=true"));
    };

    let prefs = TrapApiTokenPrefs::from_token(&new_token);
    let json = serde_json::to_string(&prefs).map_err(ApiError::internal)?;
    state
        .db
        .set_kv(TRAP_API_TOKEN_KEY, &json)
        .map_err(ApiError::internal)?;
    state.set_trap_api_token(new_token.clone());
    state.sync_trap_api_token_to_redis();

    let reveal = if body.clear {
        None
    } else {
        Some(new_token)
    };
    Ok(Json(trap_token_view(&state, reveal)))
}

fn trap_token_view(state: &AppState, reveal: Option<String>) -> TrapTokenView {
    let runtime = state
        .trap_api_token
        .read()
        .map(|g| g.trim().to_string())
        .unwrap_or_default();
    let toml = state.config.trap.api_token.trim().to_string();
    let (source, effective) = if !runtime.is_empty() {
        ("runtime", runtime)
    } else if !toml.is_empty() {
        ("toml", toml)
    } else {
        ("empty", String::new())
    };
    TrapTokenView {
        configured: !effective.is_empty(),
        token_preview: mask_token(&effective),
        token_length: effective.len(),
        source: source.into(),
        token: reveal,
        redis_synced: state.policy_redis.is_some(),
    }
}

#[derive(Debug, Serialize)]
pub struct StormSettingsView {
    #[serde(flatten)]
    pub storm: StormConfig,
    /// `runtime` (app_kv) | `toml`
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct StormSettingsUpdate {
    #[serde(flatten)]
    pub storm: StormConfig,
    /// Clear app_kv and revert to `eventide.toml` `[storm]`.
    #[serde(default)]
    pub reset: bool,
}

pub async fn get_storm(State(state): State<Arc<AppState>>) -> ApiResult<Json<StormSettingsView>> {
    Ok(Json(storm_view(&state)))
}

pub async fn put_storm(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StormSettingsUpdate>,
) -> ApiResult<Json<StormSettingsView>> {
    if body.reset {
        state
            .db
            .delete_kv(STORM_CONFIG_KEY)
            .map_err(ApiError::internal)?;
        state.set_storm_prefs(state.config.storm.clone());
        return Ok(Json(storm_view(&state)));
    }

    let storm = body.storm.normalize();
    storm.validate().map_err(ApiError::bad)?;
    let json = serde_json::to_string(&storm).map_err(ApiError::internal)?;
    state
        .db
        .set_kv(STORM_CONFIG_KEY, &json)
        .map_err(ApiError::internal)?;
    state.set_storm_prefs(storm);
    Ok(Json(storm_view(&state)))
}

fn storm_view(state: &AppState) -> StormSettingsView {
    let source = match state.db.get_kv(STORM_CONFIG_KEY) {
        Ok(Some(_)) => "runtime",
        _ => "toml",
    };
    StormSettingsView {
        storm: state.storm_prefs(),
        source: source.into(),
    }
}
