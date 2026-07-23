//! REST API for Eventide.

mod overview;

use crate::auth::{self, require_auth};
use crate::scheduler;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use eventide_core::*;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(auth::login))
        .route(
            "/api/ingress/{id}/alertmanager",
            post(crate::ingress_api::receive_alertmanager),
        )
        .route(
            "/api/ingress/{id}/generic",
            post(crate::ingress_api::receive_generic),
        )
        .route(
            "/api/ingress/{id}/push",
            post(crate::ingress_api::receive_auto),
        )
        .with_state(state.clone());

    let protected = Router::new()
        .route("/api/auth/me", get(auth::me))
        .route("/api/overview", get(overview::overview))
        .route("/api/datasources", get(list_datasources).post(create_datasource))
        .route(
            "/api/datasources/{id}",
            get(get_datasource)
                .put(update_datasource)
                .delete(delete_datasource),
        )
        .route("/api/channels", get(list_channels).post(create_channel))
        .route(
            "/api/channels/{id}",
            get(get_channel).put(update_channel).delete(delete_channel),
        )
        .route("/api/rules", get(list_rules).post(create_rule))
        .route(
            "/api/rules/{id}",
            get(get_rule).put(update_rule).delete(delete_rule),
        )
        .route("/api/rules/{id}/evaluate", post(evaluate_rule_now))
        .route("/api/alerts", get(list_alerts))
        .route("/api/alerts/{id}", get(get_alert))
        .route("/api/alerts/{id}/notifies", get(list_alert_notifies))
        .route("/api/silences", get(list_silences).post(create_silence))
        .route("/api/silences/{id}", delete(delete_silence))
        .route("/api/ingress", get(list_ingress).post(create_ingress))
        .route(
            "/api/ingress/{id}",
            get(get_ingress).put(update_ingress).delete(delete_ingress),
        )
        .route("/api/ingress/{id}/test", post(test_ingress))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth))
        .with_state(state);

    public.merge(protected)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "service": "eventide" }))
}

type ApiResult<T> = Result<T, ApiError>;

struct ApiError {
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

// ---- datasources ----

#[derive(Deserialize)]
struct DatasourceInput {
    name: String,
    #[serde(default = "default_prom")]
    kind: String,
    url: String,
    #[serde(default)]
    options: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_prom() -> String {
    "prometheus".into()
}
fn default_true() -> bool {
    true
}

async fn list_datasources(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Datasource>>> {
    state
        .db
        .list_datasources()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn get_datasource(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Datasource>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_datasource(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("datasource not found"))
}

async fn create_datasource(
    State(state): State<Arc<AppState>>,
    Json(input): Json<DatasourceInput>,
) -> ApiResult<(StatusCode, Json<Datasource>)> {
    let kind = DatasourceKind::parse(&input.kind)
        .ok_or_else(|| ApiError::bad("unsupported datasource kind"))?;
    let now = Utc::now();
    let ds = Datasource {
        id: Uuid::new_v4(),
        name: input.name,
        kind,
        url: input.url,
        options: input.options,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    };
    state.db.upsert_datasource(&ds).map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(ds)))
}

async fn update_datasource(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<DatasourceInput>,
) -> ApiResult<Json<Datasource>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_datasource(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("datasource not found"))?;
    let kind = DatasourceKind::parse(&input.kind)
        .ok_or_else(|| ApiError::bad("unsupported datasource kind"))?;
    let ds = Datasource {
        id,
        name: input.name,
        kind,
        url: input.url,
        options: input.options,
        enabled: input.enabled,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    state.db.upsert_datasource(&ds).map_err(ApiError::internal)?;
    Ok(Json(ds))
}

async fn delete_datasource(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state.db.delete_datasource(id).map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("datasource not found"))
    }
}

// ---- channels ----

#[derive(Deserialize)]
struct ChannelInput {
    name: String,
    kind: String,
    url: String,
    secret: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

async fn list_channels(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<NotifyChannel>>> {
    state.db.list_channels().map(Json).map_err(ApiError::internal)
}

async fn get_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<NotifyChannel>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_channel(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("channel not found"))
}

async fn create_channel(
    State(state): State<Arc<AppState>>,
    Json(input): Json<ChannelInput>,
) -> ApiResult<(StatusCode, Json<NotifyChannel>)> {
    let kind =
        ChannelKind::parse(&input.kind).ok_or_else(|| ApiError::bad("unsupported channel kind"))?;
    let now = Utc::now();
    let ch = NotifyChannel {
        id: Uuid::new_v4(),
        name: input.name,
        kind,
        url: input.url,
        secret: input.secret,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    };
    state.db.upsert_channel(&ch).map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(ch)))
}

async fn update_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<ChannelInput>,
) -> ApiResult<Json<NotifyChannel>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_channel(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("channel not found"))?;
    let kind =
        ChannelKind::parse(&input.kind).ok_or_else(|| ApiError::bad("unsupported channel kind"))?;
    let ch = NotifyChannel {
        id,
        name: input.name,
        kind,
        url: input.url,
        secret: input.secret,
        enabled: input.enabled,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    state.db.upsert_channel(&ch).map_err(ApiError::internal)?;
    Ok(Json(ch))
}

async fn delete_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state.db.delete_channel(id).map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("channel not found"))
    }
}

// ---- rules ----

#[derive(Deserialize)]
struct RuleInput {
    name: String,
    datasource_id: Uuid,
    expr: String,
    comparator: String,
    threshold: f64,
    #[serde(default)]
    for_seconds: u64,
    #[serde(default = "default_interval")]
    interval_seconds: u64,
    #[serde(default = "default_severity")]
    severity: String,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    annotations: BTreeMap<String, String>,
    #[serde(default)]
    channel_ids: Vec<Uuid>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_interval() -> u64 {
    30
}
fn default_severity() -> String {
    "warning".into()
}

async fn list_rules(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Rule>>> {
    state.db.list_rules().map(Json).map_err(ApiError::internal)
}

async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Rule>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_rule(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("rule not found"))
}

fn rule_from_input(id: Uuid, input: RuleInput, created_at: DateTime<Utc>) -> ApiResult<Rule> {
    let comparator =
        Comparator::parse(&input.comparator).ok_or_else(|| ApiError::bad("bad comparator"))?;
    let severity =
        Severity::parse(&input.severity).ok_or_else(|| ApiError::bad("bad severity"))?;
    Ok(Rule {
        id,
        name: input.name,
        datasource_id: input.datasource_id,
        expr: input.expr,
        comparator,
        threshold: input.threshold,
        for_seconds: input.for_seconds,
        interval_seconds: input.interval_seconds.max(5),
        severity,
        labels: input.labels,
        annotations: input.annotations,
        channel_ids: input.channel_ids,
        enabled: input.enabled,
        created_at,
        updated_at: Utc::now(),
    })
}

async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(input): Json<RuleInput>,
) -> ApiResult<(StatusCode, Json<Rule>)> {
    if state
        .db
        .get_datasource(input.datasource_id)
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(ApiError::bad("datasource_id not found"));
    }
    let rule = rule_from_input(Uuid::new_v4(), input, Utc::now())?;
    state.db.upsert_rule(&rule).map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(rule)))
}

async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<RuleInput>,
) -> ApiResult<Json<Rule>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_rule(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("rule not found"))?;
    let rule = rule_from_input(id, input, existing.created_at)?;
    state.db.upsert_rule(&rule).map_err(ApiError::internal)?;
    Ok(Json(rule))
}

async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state.db.delete_rule(id).map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("rule not found"))
    }
}

async fn evaluate_rule_now(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let id = parse_id(&id)?;
    let rule = state
        .db
        .get_rule(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("rule not found"))?;
    let now = Utc::now();
    scheduler::evaluate_one(state.clone(), &rule, now)
        .await
        .map_err(ApiError::internal)?;
    let _ = state.db.touch_rule_run(rule.id, now);
    let alerts = state
        .db
        .alerts_for_rule(rule.id)
        .map_err(ApiError::internal)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "alerts": alerts.values().cloned().collect::<Vec<_>>(),
    })))
}

// ---- alerts ----

#[derive(Deserialize)]
struct AlertQuery {
    status: Option<String>,
    severity: Option<String>,
    /// Free-text search over alertname / summary / labels / fingerprint.
    q: Option<String>,
    /// `ingress` | `rule` — filter by source label prefix or absence.
    source: Option<String>,
}

async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AlertQuery>,
) -> ApiResult<Json<Vec<AlertEvent>>> {
    let mut alerts = state
        .db
        .list_alerts(q.status.as_deref())
        .map_err(ApiError::internal)?;

    if let Some(sev) = q.severity.as_deref().filter(|s| !s.is_empty()) {
        alerts.retain(|a| a.severity.as_str() == sev);
    }
    if let Some(src) = q.source.as_deref().filter(|s| !s.is_empty()) {
        match src {
            "ingress" => alerts.retain(|a| {
                a.labels
                    .get("source")
                    .map(|s| s.starts_with("ingress:"))
                    .unwrap_or(false)
            }),
            "rule" => alerts.retain(|a| {
                !a.labels
                    .get("source")
                    .map(|s| s.starts_with("ingress:"))
                    .unwrap_or(false)
            }),
            _ => {}
        }
    }
    if let Some(needle) = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let n = needle.to_ascii_lowercase();
        alerts.retain(|a| alert_matches_query(a, &n));
    }

    // Firing first, then pending, then resolved; within group by last_evaluated desc (already mostly).
    alerts.sort_by(|a, b| {
        let rank = |s: &AlertStatus| match s {
            AlertStatus::Firing => 0,
            AlertStatus::Pending => 1,
            AlertStatus::Resolved => 2,
        };
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then_with(|| b.last_evaluated_at.cmp(&a.last_evaluated_at))
    });

    Ok(Json(alerts))
}

fn alert_matches_query(a: &AlertEvent, needle: &str) -> bool {
    let name = a
        .labels
        .get("alertname")
        .cloned()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.contains(needle) {
        return true;
    }
    if a.fingerprint.to_ascii_lowercase().contains(needle) {
        return true;
    }
    if a.labels.values().any(|v| v.to_ascii_lowercase().contains(needle)) {
        return true;
    }
    if a.annotations
        .values()
        .any(|v| v.to_ascii_lowercase().contains(needle))
    {
        return true;
    }
    false
}

async fn get_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AlertEvent>> {
    state
        .db
        .get_alert(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("alert not found"))
}

async fn list_alert_notifies(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<NotifyLog>>> {
    state
        .db
        .list_notify_logs_for_alert(id)
        .map(Json)
        .map_err(ApiError::internal)
}

#[derive(Deserialize)]
struct IngressTestInput {
    /// fire | recover | probe_fire | probe_recover
    #[serde(default = "default_scenario")]
    scenario: String,
}

fn default_scenario() -> String {
    "fire".into()
}

async fn test_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(input): Json<IngressTestInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let route = state
        .db
        .get_ingress_route(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("ingress route not found"))?;
    if !route.enabled {
        return Err(ApiError::bad("ingress is disabled"));
    }

    let payload = sample_ingress_payload(&route, &input.scenario);
    let alerts = eventide_core::parse_ingress_payload(payload.as_bytes())
        .map_err(ApiError::bad)?;
    if alerts.is_empty() {
        return Err(ApiError::bad("sample produced no alerts"));
    }
    let n = crate::ingress_api::ingest_list(&state, &route, alerts)
        .await
        .map_err(ApiError::bad)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "accepted": n,
        "scenario": input.scenario,
        "hint": "已写入告警事件，可在「告警事件」页查看"
    })))
}

fn sample_ingress_payload(route: &IngressRoute, scenario: &str) -> String {
    let name = format!("试推送·{}", route.name);
    let now_s = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let fp = format!("test-{}", route.id);
    let probe_id = format!("test-probe-{}", route.id);
    match scenario {
        "recover" => serde_json::json!({
            "alerts": [{
                "status": "resolved",
                "fingerprint": fp,
                "labels": {
                    "alertname": name,
                    "severity": "warning",
                    "instance": "eventide-test"
                },
                "annotations": {
                    "summary": "控制台试推送：恢复事件"
                },
                "severity": "warning",
                "value": 0.0
            }]
        })
        .to_string(),
        "probe_fire" => serde_json::json!({
            "eventType": "fire",
            "eventTime": now_s,
            "messageId": probe_id,
            "resultFlag": "BAD",
            "retCode": "4001",
            "retMessage": "控制台试推送：拨测失败示例",
            "retTimeMs": 1234,
            "bizname": "demo",
            "bizchainName": name,
            "alertCategory": "business",
            "areacode": "test"
        })
        .to_string(),
        "probe_recover" => serde_json::json!({
            "eventType": "recover",
            "eventTime": now_s,
            "messageId": probe_id,
            "resultFlag": "GOOD",
            "retCode": "4000",
            "retMessage": "控制台试推送：拨测恢复",
            "retTimeMs": 800,
            "bizname": "demo",
            "bizchainName": name,
            "alertCategory": "business",
            "areacode": "test"
        })
        .to_string(),
        _ => serde_json::json!({
            "alerts": [{
                "status": "firing",
                "fingerprint": fp,
                "labels": {
                    "alertname": name,
                    "severity": "critical",
                    "instance": "eventide-test"
                },
                "annotations": {
                    "summary": "控制台试推送：模拟告警触发"
                },
                "severity": "critical",
                "value": 99.0
            }]
        })
        .to_string(),
    }
}

// ---- silences ----

#[derive(Deserialize)]
struct SilenceInput {
    rule_id: Option<Uuid>,
    #[serde(default)]
    matchers: BTreeMap<String, String>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    #[serde(default)]
    comment: String,
}

async fn list_silences(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Silence>>> {
    state.db.list_silences().map(Json).map_err(ApiError::internal)
}

async fn create_silence(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SilenceInput>,
) -> ApiResult<(StatusCode, Json<Silence>)> {
    if input.ends_at <= input.starts_at {
        return Err(ApiError::bad("ends_at must be after starts_at"));
    }
    let s = Silence {
        id: Uuid::new_v4(),
        rule_id: input.rule_id,
        matchers: input.matchers,
        starts_at: input.starts_at,
        ends_at: input.ends_at,
        comment: input.comment,
        created_at: Utc::now(),
    };
    state.db.upsert_silence(&s).map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(s)))
}

async fn delete_silence(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state.db.delete_silence(id).map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("silence not found"))
    }
}

// ---- ingress routes ----

#[derive(Deserialize)]
struct IngressInput {
    name: String,
    kind: String,
    token: Option<String>,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    options: BTreeMap<String, String>,
    #[serde(default)]
    channel_ids: Vec<Uuid>,
    #[serde(default = "default_true")]
    enabled: bool,
}

async fn list_ingress(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<IngressRoute>>> {
    state
        .db
        .list_ingress_routes()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn get_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<IngressRoute>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_ingress_route(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("ingress route not found"))
}

async fn create_ingress(
    State(state): State<Arc<AppState>>,
    Json(input): Json<IngressInput>,
) -> ApiResult<(StatusCode, Json<IngressRoute>)> {
    let kind =
        IngressKind::parse(&input.kind).ok_or_else(|| ApiError::bad("unsupported ingress kind"))?;
    let now = Utc::now();
    let route = IngressRoute {
        id: Uuid::new_v4(),
        name: input.name,
        kind,
        token: input.token,
        endpoint: input.endpoint,
        options: input.options,
        channel_ids: input.channel_ids,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    };
    state
        .db
        .upsert_ingress_route(&route)
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(route)))
}

async fn update_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<IngressInput>,
) -> ApiResult<Json<IngressRoute>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_ingress_route(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("ingress route not found"))?;
    let kind =
        IngressKind::parse(&input.kind).ok_or_else(|| ApiError::bad("unsupported ingress kind"))?;
    let route = IngressRoute {
        id,
        name: input.name,
        kind,
        token: input.token,
        endpoint: input.endpoint,
        options: input.options,
        channel_ids: input.channel_ids,
        enabled: input.enabled,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    state
        .db
        .upsert_ingress_route(&route)
        .map_err(ApiError::internal)?;
    Ok(Json(route))
}

async fn delete_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state
        .db
        .delete_ingress_route(id)
        .map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("ingress route not found"))
    }
}
