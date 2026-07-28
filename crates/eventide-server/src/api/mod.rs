//! REST API for Eventide.

mod iam;
mod overview;

use crate::auth::{self, require_auth, require_route_perm};
use crate::scheduler;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
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
        .route("/api/channels/{id}/test", post(test_channel))
        .route("/api/notifies", get(list_notifies))
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
        .route("/api/enrich", get(list_enrich).post(create_enrich))
        .route("/api/enrich/preview", post(preview_enrich))
        .route(
            "/api/enrich/{id}",
            get(get_enrich).put(update_enrich).delete(delete_enrich),
        )
        .route("/api/lookups", get(list_lookups).post(create_lookup))
        .route(
            "/api/lookups/{id}",
            get(get_lookup).put(update_lookup).delete(delete_lookup),
        )
        .route("/api/ingress", get(list_ingress).post(create_ingress))
        .route(
            "/api/ingress/{id}",
            get(get_ingress).put(update_ingress).delete(delete_ingress),
        )
        .route("/api/ingress/{id}/test", post(test_ingress))
        // IAM
        .route("/api/permissions", get(iam::list_permissions))
        .route(
            "/api/departments",
            get(iam::list_departments).post(iam::create_department),
        )
        .route(
            "/api/departments/{id}",
            put(iam::update_department).delete(iam::delete_department),
        )
        .route("/api/roles", get(iam::list_roles).post(iam::create_role))
        .route(
            "/api/roles/{id}",
            put(iam::update_role).delete(iam::delete_role),
        )
        .route("/api/users", get(iam::list_users).post(iam::create_user))
        .route(
            "/api/users/{id}",
            put(iam::update_user).delete(iam::delete_user),
        )
        .route(
            "/api/users/{id}/reset-password",
            post(iam::reset_password),
        )
        // Inner: perm check; Outer: JWT auth (last layer = outermost)
        .layer(middleware::from_fn(require_route_perm))
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
    #[serde(default)]
    options: BTreeMap<String, String>,
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
        options: input.options,
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
        options: input.options,
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

async fn test_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let id = parse_id(&id)?;
    let ch = state
        .db
        .get_channel(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("channel not found"))?;
    let text = format!(
        "[Eventide 渠道测试]\n渠道: {}\n类型: {}\n时间: {}\n若收到此消息，说明通知渠道配置正常。",
        ch.name,
        ch.kind.as_str(),
        Utc::now().to_rfc3339()
    );
    let report = state.notifier.send_text_report(&ch, &text).await;
    let now = Utc::now();
    let _ = state.db.insert_notify_log(&NotifyLog {
        id: Uuid::new_v4(),
        alert_id: Uuid::nil(),
        channel_id: ch.id,
        transition: AlertTransition::Unchanged,
        success: report.ok,
        error: report
            .error
            .clone()
            .or_else(|| Some("test".into())),
        body: text.clone(),
        created_at: now,
    });
    Ok(Json(serde_json::json!({
        "ok": report.ok,
        "hint": if report.ok {
            "测试消息已发送，请到对应群 / Webhook 查收"
        } else {
            "发送失败，请查看下方请求与返回"
        },
        "text": text,
        "kind": report.kind,
        "request_url": report.request_url,
        "request_body": report.request_body,
        "http_status": report.http_status,
        "response_body": report.response_body,
        "error": report.error,
    })))
}

#[derive(Deserialize)]
struct NotifyListQuery {
    channel_id: Option<String>,
    /// true | false | 1 | 0
    success: Option<String>,
    q: Option<String>,
    #[serde(default = "default_notify_limit")]
    limit: usize,
}

fn default_notify_limit() -> usize {
    100
}

async fn list_notifies(
    State(state): State<Arc<AppState>>,
    Query(q): Query<NotifyListQuery>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let channel_id = match q.channel_id.as_deref() {
        Some(s) if !s.is_empty() => Some(parse_id(s)?),
        _ => None,
    };
    let success = match q.success.as_deref() {
        Some("1") | Some("true") | Some("ok") => Some(true),
        Some("0") | Some("false") | Some("fail") => Some(false),
        _ => None,
    };
    let logs = state
        .db
        .list_notify_logs(channel_id, success, q.q.as_deref(), q.limit)
        .map_err(ApiError::internal)?;
    let channels = state.db.list_channels().map_err(ApiError::internal)?;
    let ch_map: BTreeMap<_, _> = channels.into_iter().map(|c| (c.id, c)).collect();
    let out: Vec<serde_json::Value> = logs
        .into_iter()
        .map(|log| {
            let ch = ch_map.get(&log.channel_id);
            serde_json::json!({
                "id": log.id,
                "alert_id": log.alert_id,
                "channel_id": log.channel_id,
                "channel_name": ch.map(|c| c.name.as_str()).unwrap_or(""),
                "channel_kind": ch.map(|c| c.kind.as_str()).unwrap_or(""),
                "transition": log.transition,
                "success": log.success,
                "error": log.error,
                "body": log.body,
                "created_at": log.created_at,
            })
        })
        .collect();
    Ok(Json(out))
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
    let alerts = eventide_core::parse_ingress_payload_with_options(payload.as_bytes(), &route.options)
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
    // Zabbix / custom field-mapping sample (matches Demo Zabbix Kafka options).
    let mapped = route.options.get("map_name").map(|s| s.as_str()) == Some("sourcealertkey")
        || route
            .options
            .get("map_ip")
            .map(|s| s.contains("sourceciname"))
            .unwrap_or(false);
    if mapped {
        let status = match scenario {
            "recover" | "probe_recover" => 0,
            _ => 2,
        };
        return serde_json::json!({
            "severity": 4,
            "summary": "麒麟主机当前系统磁盘[vdb] IO使用百分比为: 97.49 %, 已超过90%阈值",
            "lastoccurrence": now_s,
            "status": status,
            "sourceid": 1,
            "sourceeventid": "71978",
            "sourceciname": "82.12.161.32_kylin",
            "sourcealertkey": "vfs.dev.util[vdb]",
            "sourceseverity": "High",
            "sourceidentifier": "82.12.161.32_kylin_vfs.dev.util[vdb]_Application:Disk vdb",
            "ciinstance": "Application:Disk vdb",
            "eventtypeid": "*UNKNOWN*"
        })
        .to_string();
    }
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

// ---- enrich rules ----

#[derive(Deserialize)]
struct EnrichInput {
    name: String,
    #[serde(default = "default_enrich_kind")]
    #[allow(dead_code)]
    kind: String,
    #[serde(default)]
    matchers: BTreeMap<String, String>,
    #[serde(default)]
    match_key: String,
    #[serde(default)]
    templates: BTreeMap<String, String>,
    #[serde(default)]
    mappings: BTreeMap<String, BTreeMap<String, String>>,
    /// Preferred: one rule may reference many lookup tables.
    #[serde(default)]
    lookup_table_ids: Vec<Uuid>,
    /// Legacy single-id field; merged into `lookup_table_ids` when present.
    #[serde(default)]
    lookup_table_id: Option<Uuid>,
    /// Per-table match label for this rule: table uuid → label name (e.g. sss_ip).
    #[serde(default)]
    lookup_match_keys: BTreeMap<String, String>,
    /// First-class field overrides: severity / ip / alertname / summary.
    #[serde(default)]
    field_templates: BTreeMap<String, String>,
    /// Before lookup: write labels from templates (supports `|before:` etc.).
    #[serde(default)]
    label_extracts: BTreeMap<String, String>,
    #[serde(default)]
    write_labels: bool,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_priority")]
    priority: i32,
}

fn default_enrich_kind() -> String {
    "auto".into()
}

fn default_priority() -> i32 {
    100
}

fn enrich_from_input(
    id: Uuid,
    input: EnrichInput,
    created_at: DateTime<Utc>,
) -> ApiResult<EnrichRule> {
    let mut lookup_table_ids = input.lookup_table_ids;
    if let Some(tid) = input.lookup_table_id {
        if !lookup_table_ids.contains(&tid) {
            lookup_table_ids.push(tid);
        }
    }
    let mut seen = std::collections::HashSet::new();
    lookup_table_ids.retain(|u| seen.insert(*u));

    let has_tpl = !input.templates.is_empty();
    let has_fields = !input.field_templates.is_empty();
    let has_extracts = !input.label_extracts.is_empty();
    let has_map = !input.match_key.trim().is_empty() && !input.mappings.is_empty();
    let has_lookup = !lookup_table_ids.is_empty();
    if !has_tpl && !has_fields && !has_extracts && !has_map && !has_lookup {
        return Err(ApiError::bad(
            "enrich rule needs at least one of: templates, field_templates, label_extracts, mappings(+match_key), or lookup_table_ids",
        ));
    }
    if !input.mappings.is_empty() && input.match_key.trim().is_empty() {
        return Err(ApiError::bad("mappings require match_key"));
    }

    let kind = EnrichRule::infer_kind(
        &input.templates,
        &input.mappings,
        &lookup_table_ids,
        &input.match_key,
        &input.field_templates,
        &input.label_extracts,
    );

    Ok(EnrichRule {
        id,
        name: input.name,
        kind,
        matchers: input.matchers,
        match_key: input.match_key,
        templates: input.templates,
        mappings: input.mappings,
        lookup_table_ids,
        lookup_match_keys: input.lookup_match_keys,
        field_templates: input.field_templates,
        label_extracts: input.label_extracts,
        write_labels: input.write_labels,
        enabled: input.enabled,
        priority: input.priority,
        created_at,
        updated_at: Utc::now(),
    })
}

async fn list_enrich(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<EnrichRule>>> {
    state
        .db
        .list_enrich_rules()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn get_enrich(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<EnrichRule>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_enrich_rule(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("enrich rule not found"))
}

async fn create_enrich(
    State(state): State<Arc<AppState>>,
    Json(input): Json<EnrichInput>,
) -> ApiResult<(StatusCode, Json<EnrichRule>)> {
    let rule = enrich_from_input(Uuid::new_v4(), input, Utc::now())?;
    state
        .db
        .upsert_enrich_rule(&rule)
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(rule)))
}

async fn update_enrich(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<EnrichInput>,
) -> ApiResult<Json<EnrichRule>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_enrich_rule(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("enrich rule not found"))?;
    let rule = enrich_from_input(id, input, existing.created_at)?;
    state
        .db
        .upsert_enrich_rule(&rule)
        .map_err(ApiError::internal)?;
    Ok(Json(rule))
}

async fn delete_enrich(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state
        .db
        .delete_enrich_rule(id)
        .map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("enrich rule not found"))
    }
}

#[derive(Deserialize)]
struct EnrichPreviewInput {
    /// Optional: apply only this rule draft (not yet saved).
    #[serde(default)]
    rule: Option<EnrichInput>,
    /// Or apply all saved rules when `rule` is omitted.
    #[serde(default)]
    use_saved: bool,
    /// Flat labels (legacy). Ignored when `payload` is set.
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    annotations: BTreeMap<String, String>,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    rule_name: Option<String>,
    /// Raw ingress JSON (Kafka / Generic / Alertmanager). Preferred for preview.
    #[serde(default)]
    payload: Option<serde_json::Value>,
    /// Use this ingress route's field mapping to parse `payload`.
    #[serde(default)]
    ingress_id: Option<Uuid>,
}

async fn preview_enrich(
    State(state): State<Arc<AppState>>,
    Json(input): Json<EnrichPreviewInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let (mut event, parsed_via) = preview_event_from_input(state.as_ref(), &input)?;

    let before = serde_json::json!({
        "labels": event.labels.clone(),
        "annotations": event.annotations.clone(),
        "severity": event.severity.as_str(),
    });

    let mut rules = Vec::new();
    if let Some(draft) = input.rule {
        rules.push(enrich_from_input(Uuid::nil(), draft, Utc::now())?);
    } else if input.use_saved {
        rules = state.db.list_enrich_rules().map_err(ApiError::internal)?;
    }

    let lookups = state.db.lookup_tables_map().map_err(ApiError::internal)?;
    enrich_alert(&mut event, input.rule_name.as_deref(), &rules, &lookups);
    Ok(Json(serde_json::json!({
        "labels": event.labels,
        "annotations": event.annotations,
        "severity": event.severity.as_str(),
        "parsed_via": parsed_via,
        "before": before,
    })))
}

fn preview_event_from_input(
    state: &AppState,
    input: &EnrichPreviewInput,
) -> ApiResult<(AlertEvent, String)> {
    if let Some(payload) = &input.payload {
        let options = if let Some(id) = input.ingress_id {
            let route = state
                .db
                .get_ingress_route(id)
                .map_err(ApiError::internal)?
                .ok_or_else(|| ApiError::not_found("ingress route not found"))?;
            route.options
        } else {
            BTreeMap::new()
        };
        let raw = serde_json::to_vec(payload).map_err(|e| ApiError::bad(e.to_string()))?;
        match parse_ingress_payload_with_options(&raw, &options) {
            Ok(alerts) => {
                let incoming = alerts.into_iter().next().ok_or_else(|| {
                    ApiError::bad("payload produced no alerts (check JSON / 字段映射)")
                })?;
                let via = if input.ingress_id.is_some() {
                    "ingress_mapping"
                } else {
                    "builtin_parser"
                };
                return Ok((alert_event_from_ingress(&incoming), via.into()));
            }
            Err(parse_err) => {
                // Fallback: flat labels object {"ip":"...","alertname":"..."}
                if looks_like_external_alert(payload) {
                    return Err(ApiError::bad(format!(
                        "{parse_err}（这是接入原始 JSON，请在试跑里选择对应的「字段映射接入」）"
                    )));
                }
                if let Some(labels) = flat_string_map(payload) {
                    let severity = input
                        .severity
                        .as_deref()
                        .and_then(Severity::parse)
                        .or_else(|| labels.get("severity").and_then(|s| Severity::parse(s)))
                        .unwrap_or(Severity::Warning);
                    return Ok((
                        AlertEvent {
                            id: Uuid::nil(),
                            rule_id: Uuid::nil(),
                            fingerprint: "preview".into(),
                            status: AlertStatus::Firing,
                            severity,
                            labels,
                            annotations: input.annotations.clone(),
                            value: input.value,
                            starts_at: Utc::now(),
                            ends_at: None,
                            pending_since: None,
                            last_evaluated_at: Utc::now(),
                            notified_firing: false,
                            notified_resolved: false,
                        },
                        "labels".into(),
                    ));
                }
                return Err(ApiError::bad(format!(
                    "{parse_err}（Kafka/自定义 JSON 请选择带字段映射的接入）"
                )));
            }
        }
    }

    let severity = input
        .severity
        .as_deref()
        .and_then(Severity::parse)
        .unwrap_or(Severity::Warning);
    Ok((
        AlertEvent {
            id: Uuid::nil(),
            rule_id: Uuid::nil(),
            fingerprint: "preview".into(),
            status: AlertStatus::Firing,
            severity,
            labels: input.labels.clone(),
            annotations: input.annotations.clone(),
            value: input.value,
            starts_at: Utc::now(),
            ends_at: None,
            pending_since: None,
            last_evaluated_at: Utc::now(),
            notified_firing: false,
            notified_resolved: false,
        },
        "labels".into(),
    ))
}

fn flat_string_map(v: &serde_json::Value) -> Option<BTreeMap<String, String>> {
    let obj = v.as_object()?;
    if obj.is_empty() {
        return None;
    }
    let mut out = BTreeMap::new();
    for (k, val) in obj {
        let s = match val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => continue,
            _ => return None,
        };
        out.insert(k.clone(), s);
    }
    Some(out)
}

fn looks_like_external_alert(v: &serde_json::Value) -> bool {
    let Some(obj) = v.as_object() else {
        return v.get("alerts").is_some();
    };
    [
        "sourcealertkey",
        "sourceidentifier",
        "sourceciname",
        "eventType",
        "alerts",
        "status",
        "summary",
    ]
    .iter()
    .filter(|k| obj.contains_key(**k))
    .count()
        >= 2
}

fn alert_event_from_ingress(incoming: &IngressAlert) -> AlertEvent {
    AlertEvent {
        id: Uuid::nil(),
        rule_id: Uuid::nil(),
        fingerprint: incoming
            .fingerprint
            .clone()
            .unwrap_or_else(|| "preview".into()),
        status: incoming.status,
        severity: incoming.severity,
        labels: incoming.labels.clone(),
        annotations: incoming.annotations.clone(),
        value: incoming.value,
        starts_at: incoming.starts_at.unwrap_or_else(Utc::now),
        ends_at: incoming.ends_at,
        pending_since: None,
        last_evaluated_at: Utc::now(),
        notified_firing: false,
        notified_resolved: false,
    }
}

// ---- lookup tables ----

#[derive(Deserialize)]
struct LookupInput {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_lookup_key")]
    key_label: String,
    /// Structured rows (JSON object). Preferred when non-empty.
    #[serde(default)]
    rows: BTreeMap<String, BTreeMap<String, String>>,
    /// Omnibus / `.lookup` 原文，或 JSON 对象字符串；仅在 `rows` 为空时解析。
    #[serde(default)]
    text: Option<String>,
    /// When true and `text` is `.lookup` format, override `key_label` from header.
    #[serde(default)]
    sync_key_from_text: bool,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_lookup_key() -> String {
    "instance".into()
}

fn lookup_from_input(
    id: Uuid,
    input: LookupInput,
    created_at: DateTime<Utc>,
) -> ApiResult<LookupTable> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad("name required"));
    }
    let mut key_label = input.key_label;
    // Prefer structured rows from the client when present (avoids empty saves when
    // the browser already parsed .lookup text, or the server binary is stale).
    let rows = if !input.rows.is_empty() {
        input.rows
    } else if let Some(text) = input.text.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let (parsed_key, rows) =
            parse_lookup_rows_auto(text).map_err(|e| ApiError::bad(e))?;
        if input.sync_key_from_text {
            if let Some(k) = parsed_key {
                if !k.is_empty() {
                    key_label = k;
                }
            }
        }
        rows
    } else {
        BTreeMap::new()
    };
    if rows.is_empty() {
        return Err(ApiError::bad(
            "lookup rows empty：请检查外表内容是否已正确解析（列需用空格或 Tab 分隔）",
        ));
    }
    if key_label.trim().is_empty() {
        return Err(ApiError::bad("key_label required"));
    }
    Ok(LookupTable {
        id,
        name: input.name,
        description: input.description,
        key_label,
        rows,
        enabled: input.enabled,
        created_at,
        updated_at: Utc::now(),
    })
}

async fn list_lookups(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<LookupTable>>> {
    state
        .db
        .list_lookup_tables()
        .map(Json)
        .map_err(ApiError::internal)
}

async fn get_lookup(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<LookupTable>> {
    let id = parse_id(&id)?;
    state
        .db
        .get_lookup_table(id)
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("lookup table not found"))
}

async fn create_lookup(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LookupInput>,
) -> ApiResult<(StatusCode, Json<LookupTable>)> {
    let table = lookup_from_input(Uuid::new_v4(), input, Utc::now())?;
    state
        .db
        .upsert_lookup_table(&table)
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(table)))
}

async fn update_lookup(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<LookupInput>,
) -> ApiResult<Json<LookupTable>> {
    let id = parse_id(&id)?;
    let existing = state
        .db
        .get_lookup_table(id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("lookup table not found"))?;
    let table = lookup_from_input(id, input, existing.created_at)?;
    state
        .db
        .upsert_lookup_table(&table)
        .map_err(ApiError::internal)?;
    Ok(Json(table))
}

async fn delete_lookup(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id = parse_id(&id)?;
    let ok = state
        .db
        .delete_lookup_table(id)
        .map_err(ApiError::internal)?;
    if ok {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("lookup table not found"))
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
