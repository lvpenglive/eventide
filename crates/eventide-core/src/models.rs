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

/// Alert severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "info" => Some(Self::Info),
            "warning" | "warn" => Some(Self::Warning),
            "critical" | "crit" => Some(Self::Critical),
            _ => None,
        }
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
}

impl ChannelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::DingTalk => "dingtalk",
            Self::WeCom => "wecom",
            Self::Feishu => "feishu",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "webhook" => Some(Self::Webhook),
            "dingtalk" | "ding" => Some(Self::DingTalk),
            "wecom" | "wechat" | "qywx" | "企业微信" => Some(Self::WeCom),
            "feishu" | "lark" | "飞书" => Some(Self::Feishu),
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
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub notified_firing: bool,
    pub notified_resolved: bool,
}

/// Transition emitted by the alert state machine (edge for notifications).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertTransition {
    /// Condition met long enough: pending/new → firing.
    BecameFiring,
    /// Condition cleared: firing → resolved.
    BecameResolved,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyChannel {
    pub id: Uuid,
    pub name: String,
    pub kind: ChannelKind,
    /// Webhook / DingTalk / WeCom / Feishu robot URL.
    pub url: String,
    /// Optional secret for signed robots (DingTalk / Feishu).
    pub secret: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
}

impl Silence {
    /// Returns true if this silence applies to the given rule + labels at `now`.
    pub fn matches(&self, rule_id: Uuid, labels: &Labels, now: DateTime<Utc>) -> bool {
        if now < self.starts_at || now >= self.ends_at {
            return false;
        }
        if let Some(rid) = self.rule_id {
            if rid != rule_id {
                return false;
            }
        }
        for (k, v) in &self.matchers {
            match labels.get(k) {
                Some(lv) if lv == v => {}
                _ => return false,
            }
        }
        true
    }
}
