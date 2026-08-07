//! Domain models for datasources, rules, alerts, silences, and notify channels.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Comparison operator for threshold rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

impl Comparator {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Eq => "==",
            Self::Neq => "!=",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            ">" | "gt" => Some(Self::Gt),
            ">=" | "gte" => Some(Self::Gte),
            "<" | "lt" => Some(Self::Lt),
            "<=" | "lte" => Some(Self::Lte),
            "==" | "=" | "eq" => Some(Self::Eq),
            "!=" | "neq" => Some(Self::Neq),
            _ => None,
        }
    }
}

/// Alert severity (aligned with Zabbix 0–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 0 — Not classified
    NotClassified,
    /// 1 — Information
    Information,
    /// 2 — Warning
    Warning,
    /// 3 — Average
    Average,
    /// 4 — High
    High,
    /// 5 — Disaster
    Disaster,
}

impl Severity {
    pub const ALL: [Severity; 6] = [
        Self::NotClassified,
        Self::Information,
        Self::Warning,
        Self::Average,
        Self::High,
        Self::Disaster,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotClassified => "not_classified",
            Self::Information => "information",
            Self::Warning => "warning",
            Self::Average => "average",
            Self::High => "high",
            Self::Disaster => "disaster",
        }
    }

    /// Zabbix numeric level 0–5.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::NotClassified => 0,
            Self::Information => 1,
            Self::Warning => 2,
            Self::Average => 3,
            Self::High => 4,
            Self::Disaster => 5,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        // Numeric 0–5 (Zabbix)
        if let Ok(n) = t.parse::<u8>() {
            return match n {
                0 => Some(Self::NotClassified),
                1 => Some(Self::Information),
                2 => Some(Self::Warning),
                3 => Some(Self::Average),
                4 => Some(Self::High),
                5 => Some(Self::Disaster),
                _ => None,
            };
        }
        match t.to_ascii_lowercase().as_str() {
            "not_classified" | "notclassified" | "unknown" | "none" => {
                Some(Self::NotClassified)
            }
            "information" | "info" | "informational" => Some(Self::Information),
            "warning" | "warn" => Some(Self::Warning),
            "average" | "avg" | "minor" => Some(Self::Average),
            "high" | "major" => Some(Self::High),
            "disaster" | "critical" | "crit" | "fatal" | "emergency" => Some(Self::Disaster),
            _ => None,
        }
    }
}

impl Serialize for Severity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Severity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Severity::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown severity: {s}")))
    }
}

/// Lifecycle state of an alert event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertStatus {
    Pending,
    Firing,
    Resolved,
}

impl AlertStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Firing => "firing",
            Self::Resolved => "resolved",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "firing" => Some(Self::Firing),
            "resolved" => Some(Self::Resolved),
            _ => None,
        }
    }
}

/// Supported datasource kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasourceKind {
    Prometheus,
    /// VictoriaMetrics — PromQL-compatible HTTP API.
    VictoriaMetrics,
    /// Kafka brokers — evaluate topic depth / recent message count.
    Kafka,
    /// Log backend (Loki LogQL) — evaluate log query results as samples.
    Log,
}

impl DatasourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prometheus => "prometheus",
            Self::VictoriaMetrics => "victoriametrics",
            Self::Kafka => "kafka",
            Self::Log => "log",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "prometheus" | "prom" => Some(Self::Prometheus),
            "victoriametrics" | "victoria" | "vm" => Some(Self::VictoriaMetrics),
            "kafka" => Some(Self::Kafka),
            "log" | "loki" | "logs" => Some(Self::Log),
            _ => None,
        }
    }

    /// Prometheus and VictoriaMetrics speak the PromQL instant-query API.
    pub fn uses_promql_http(self) -> bool {
        matches!(self, Self::Prometheus | Self::VictoriaMetrics)
    }
}

/// Notify channel kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Webhook,
    DingTalk,
    WeCom,
    Feishu,
    /// Slack Incoming Webhook.
    Slack,
    /// Telegram Bot API (`sendMessage`).
    Telegram,
    /// Custom HTTP JSON POST/PUT (user-defined body template).
    Http,
}

impl ChannelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::DingTalk => "dingtalk",
            Self::WeCom => "wecom",
            Self::Feishu => "feishu",
            Self::Slack => "slack",
            Self::Telegram => "telegram",
            Self::Http => "http",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "webhook" => Some(Self::Webhook),
            "dingtalk" | "ding" => Some(Self::DingTalk),
            "wecom" | "wechat" | "qywx" | "企业微信" => Some(Self::WeCom),
            "feishu" | "lark" | "飞书" => Some(Self::Feishu),
            "slack" => Some(Self::Slack),
            "telegram" | "tg" => Some(Self::Telegram),
            "http" | "custom" | "http_json" | "json" => Some(Self::Http),
            _ => None,
        }
    }
}

/// Ingress webhook / stream kinds (push alerts from external platforms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngressKind {
    Alertmanager,
    Generic,
    /// Consume alert JSON from a Kafka topic.
    Kafka,
}

impl IngressKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alertmanager => "alertmanager",
            Self::Generic => "generic",
            Self::Kafka => "kafka",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "alertmanager" | "am" => Some(Self::Alertmanager),
            "generic" | "webhook" => Some(Self::Generic),
            "kafka" => Some(Self::Kafka),
            _ => None,
        }
    }
}

pub type Labels = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Datasource {
    pub id: Uuid,
    pub name: String,
    pub kind: DatasourceKind,
    /// Endpoint: Prom/Loki base URL, or Kafka brokers `host:9092,host2:9092`.
    pub url: String,
    /// Kind-specific options (e.g. kafka topic, log backend type).
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: Uuid,
    pub name: String,
    pub datasource_id: Uuid,
    /// Query expression: PromQL / LogQL / Kafka topic override.
    pub expr: String,
    pub comparator: Comparator,
    pub threshold: f64,
    /// How long the condition must hold before firing (seconds).
    pub for_seconds: u64,
    /// Evaluation / poll interval in seconds.
    pub interval_seconds: u64,
    pub severity: Severity,
    pub labels: Labels,
    pub annotations: Labels,
    /// Channel IDs that receive notifications for this rule.
    pub channel_ids: Vec<Uuid>,
    /// Seconds after firing `starts_at` without ack before escalation notify (`0` = off).
    #[serde(default)]
    pub escalate_after_seconds: u64,
    /// Optional severity bump when escalating (only applied if higher than current).
    #[serde(default)]
    pub escalate_severity: Option<Severity>,
    /// Channels for escalation; empty → fall back to `channel_ids`.
    #[serde(default)]
    pub escalate_channel_ids: Vec<Uuid>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Rule {
    pub fn escalate_notify_channels(&self) -> &[Uuid] {
        if self.escalate_channel_ids.is_empty() {
            &self.channel_ids
        } else {
            &self.escalate_channel_ids
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub fingerprint: String,
    pub status: AlertStatus,
    pub severity: Severity,
    pub labels: Labels,
    pub annotations: Labels,
    pub value: Option<f64>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    /// When the condition first became true while pending.
    pub pending_since: Option<DateTime<Utc>>,
    pub last_evaluated_at: DateTime<Utc>,
    /// Dedup occurrence count for the current episode (Netcool-style Tally).
    #[serde(default = "default_tally")]
    pub tally: u32,
    /// Last time this fingerprint was seen while still active (distinct from last_evaluated_at).
    #[serde(default = "utc_now_default")]
    pub last_occurrence_at: DateTime<Utc>,
    pub notified_firing: bool,
    pub notified_resolved: bool,
    /// Operator acknowledge / take-ownership (independent of lifecycle status).
    #[serde(default)]
    pub acknowledged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub acknowledged_by: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub ack_comment: Option<String>,
    /// Manual close (force resolve) — for sources without recover.
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_by: Option<String>,
    #[serde(default)]
    pub close_comment: Option<String>,
    /// When an unacked timeout escalation notify was sent for this episode.
    #[serde(default)]
    pub escalated_at: Option<DateTime<Utc>>,
}

fn default_tally() -> u32 {
    1
}

fn utc_now_default() -> DateTime<Utc> {
    Utc::now()
}

impl AlertEvent {
    pub fn clear_ack(&mut self) {
        self.acknowledged_at = None;
        self.acknowledged_by = None;
        self.assignee = None;
        self.ack_comment = None;
    }

    pub fn clear_close(&mut self) {
        self.closed_at = None;
        self.closed_by = None;
        self.close_comment = None;
    }

    pub fn clear_escalation(&mut self) {
        self.escalated_at = None;
    }

    pub fn is_acknowledged(&self) -> bool {
        self.acknowledged_at.is_some()
    }

    pub fn is_manually_closed(&self) -> bool {
        self.closed_at.is_some()
    }

    /// Start (or restart) an episode: tally=1 at `now`.
    pub fn reset_occurrence(&mut self, now: DateTime<Utc>) {
        self.tally = 1;
        self.last_occurrence_at = now;
    }

    /// Same episode seen again while still active.
    pub fn bump_occurrence(&mut self, now: DateTime<Utc>) {
        self.tally = self.tally.saturating_add(1).max(1);
        self.last_occurrence_at = now;
    }
}

/// Transition emitted by the alert state machine (edge for notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertTransition {
    /// Condition met long enough: pending/new → firing.
    BecameFiring,
    /// Condition cleared: firing → resolved.
    BecameResolved,
    /// Unacked timeout escalation (independent of notified_firing).
    Escalated,
    /// Still pending / still firing / still resolved — no notify edge.
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Silence {
    pub id: Uuid,
    pub rule_id: Option<Uuid>,
    pub matchers: Labels,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub comment: String,
    pub created_at: DateTime<Utc>,
}

/// Planned maintenance window — suppress notifications like Silence, with enable flag + name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceWindow {
    pub id: Uuid,
    pub name: String,
    pub comment: String,
    pub rule_id: Option<Uuid>,
    pub matchers: Labels,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Exact AND match of every matcher key against labels.
pub fn labels_matchers_apply(matchers: &Labels, labels: &Labels) -> bool {
    for (k, v) in matchers {
        match labels.get(k) {
            Some(lv) if lv == v => {}
            _ => return false,
        }
    }
    true
}

/// Half-open window `[starts_at, ends_at)` plus optional rule_id + label matchers.
pub fn suppress_window_matches(
    rule_id_filter: Option<Uuid>,
    matchers: &Labels,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    rule_id: Uuid,
    labels: &Labels,
    now: DateTime<Utc>,
) -> bool {
    if now < starts_at || now >= ends_at {
        return false;
    }
    if let Some(rid) = rule_id_filter {
        if rid != rule_id {
            return false;
        }
    }
    labels_matchers_apply(matchers, labels)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyChannel {
    pub id: Uuid,
    pub name: String,
    pub kind: ChannelKind,
    /// Webhook / DingTalk / WeCom / Feishu robot URL.
    pub url: String,
    /// Optional secret for signed robots (DingTalk / Feishu).
    pub secret: Option<String>,
    /// Channel options (e.g. `template_firing` / `template_resolved` / `template`).
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NotifyChannel {
    /// Resolve notify body template for a transition.
    /// Prefers `template_firing` / `template_resolved`; falls back to `template`.
    pub fn notify_template(&self, transition: AlertTransition) -> Option<&str> {
        let specific = match transition {
            AlertTransition::BecameFiring => self.options.get("template_firing"),
            AlertTransition::BecameResolved => self.options.get("template_resolved"),
            AlertTransition::Escalated => self
                .options
                .get("template_escalated")
                .or_else(|| self.options.get("template_firing")),
            AlertTransition::Unchanged => self
                .options
                .get("template_firing")
                .or_else(|| self.options.get("template")),
        };
        specific
            .or_else(|| self.options.get("template"))
            .map(|s| s.as_str())
            .filter(|s| !s.trim().is_empty())
    }

    /// Custom HTTP JSON body template (`json_firing` / `json_resolved` / `json_body`).
    pub fn json_template(&self, transition: AlertTransition) -> Option<&str> {
        let specific = match transition {
            AlertTransition::BecameFiring => self.options.get("json_firing"),
            AlertTransition::BecameResolved => self.options.get("json_resolved"),
            AlertTransition::Escalated => self
                .options
                .get("json_escalated")
                .or_else(|| self.options.get("json_firing")),
            AlertTransition::Unchanged => self
                .options
                .get("json_firing")
                .or_else(|| self.options.get("json_body")),
        };
        specific
            .or_else(|| self.options.get("json_body"))
            .map(|s| s.as_str())
            .filter(|s| !s.trim().is_empty())
    }
}

/// Push-alert ingress route (Alertmanager / generic webhook / Kafka).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressRoute {
    pub id: Uuid,
    pub name: String,
    pub kind: IngressKind,
    /// Optional bearer token (HTTP ingress); unused for Kafka.
    pub token: Option<String>,
    /// Kafka brokers or extra endpoint (optional for HTTP kinds).
    #[serde(default)]
    pub endpoint: String,
    /// Kind-specific options (e.g. kafka topic).
    #[serde(default)]
    pub options: BTreeMap<String, String>,
    pub channel_ids: Vec<Uuid>,
    /// Seconds after firing without ack before escalation (`0` = off). Same semantics as Rule.
    #[serde(default)]
    pub escalate_after_seconds: u64,
    #[serde(default)]
    pub escalate_severity: Option<Severity>,
    #[serde(default)]
    pub escalate_channel_ids: Vec<Uuid>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One alert pushed from an external platform (pre-normalization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressAlert {
    pub status: AlertStatus,
    pub fingerprint: Option<String>,
    pub labels: Labels,
    pub annotations: Labels,
    pub severity: Severity,
    pub value: Option<f64>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyLog {
    pub id: Uuid,
    pub alert_id: Uuid,
    pub channel_id: Uuid,
    pub transition: AlertTransition,
    pub success: bool,
    pub error: Option<String>,
    /// Plain-text (or JSON) payload that was / would be sent.
    #[serde(default)]
    pub body: String,
    pub created_at: DateTime<Utc>,
}

/// Console / API mutation audit trail (who did what).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub actor_username: String,
    pub actor_uid: Option<Uuid>,
    /// e.g. `alert.ack`, `rule.create`, `user.reset_password`
    pub action: String,
    /// e.g. `alert`, `rule`, `silence`
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    #[serde(default)]
    pub detail_json: Option<String>,
    pub client_ip: Option<String>,
}

impl Silence {
    /// Returns true if this silence applies to the given rule + labels at `now`.
    pub fn matches(&self, rule_id: Uuid, labels: &Labels, now: DateTime<Utc>) -> bool {
        suppress_window_matches(
            self.rule_id,
            &self.matchers,
            self.starts_at,
            self.ends_at,
            rule_id,
            labels,
            now,
        )
    }
}

impl MaintenanceWindow {
    /// Returns true if this enabled window applies to the given rule + labels at `now`.
    pub fn matches(&self, rule_id: Uuid, labels: &Labels, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }
        suppress_window_matches(
            self.rule_id,
            &self.matchers,
            self.starts_at,
            self.ends_at,
            rule_id,
            labels,
            now,
        )
    }
}

#[cfg(test)]
mod suppress_tests {
    use super::*;
    use chrono::TimeZone;

    fn labels(pairs: &[(&str, &str)]) -> Labels {
        pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn silence_and_mw_share_matcher_semantics() {
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let rule = Uuid::nil();
        let m = labels(&[("ip", "10.0.0.1")]);
        let silence = Silence {
            id: Uuid::nil(),
            rule_id: None,
            matchers: m.clone(),
            starts_at: t0,
            ends_at: t1,
            comment: String::new(),
            created_at: t0,
        };
        let mw = MaintenanceWindow {
            id: Uuid::nil(),
            name: "mw".into(),
            comment: String::new(),
            rule_id: None,
            matchers: m,
            starts_at: t0,
            ends_at: t1,
            enabled: true,
            created_at: t0,
            updated_at: t0,
        };
        let hit = labels(&[("ip", "10.0.0.1"), ("alertname", "x")]);
        let miss = labels(&[("ip", "10.0.0.2")]);
        let mid = t0 + chrono::Duration::hours(1);
        assert!(silence.matches(rule, &hit, mid));
        assert!(mw.matches(rule, &hit, mid));
        assert!(!silence.matches(rule, &miss, mid));
        assert!(!mw.matches(rule, &miss, mid));
        assert!(!mw.matches(rule, &hit, t1)); // half-open end
    }

    #[test]
    fn disabled_mw_never_matches() {
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let mw = MaintenanceWindow {
            id: Uuid::nil(),
            name: "off".into(),
            comment: String::new(),
            rule_id: None,
            matchers: Labels::new(),
            starts_at: t0,
            ends_at: t1,
            enabled: false,
            created_at: t0,
            updated_at: t0,
        };
        assert!(!mw.matches(Uuid::nil(), &Labels::new(), t0 + chrono::Duration::minutes(30)));
    }
}
