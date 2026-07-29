//! Alert history settings API (ES connection + write toggle + search store).

use crate::alert_history::ALERT_HISTORY_KEY;
use crate::state::AppState;
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
