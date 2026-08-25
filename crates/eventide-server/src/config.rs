//! Application configuration (TOML).

use eventide_core::{AggregateConfig, AggregateMode, IngressPressureConfig, ThrottleConfig};
use crate::leader::ClusterConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    /// MySQL connection URL, e.g. `mysql://user:pass@host:3306/eventide`
    #[serde(default = "default_mysql")]
    pub mysql_url: String,
    /// Redis connection URL, e.g. `redis://host:6379/`
    #[serde(default = "default_redis")]
    pub redis_url: String,
    #[serde(default = "default_static")]
    pub static_dir: String,
    #[serde(default = "default_v2_static")]
    pub v2_static_dir: Option<String>,
    /// Global scheduler tick in seconds (rules still have their own interval).
    #[serde(default = "default_tick")]
    pub scheduler_tick_seconds: u64,
    #[serde(default)]
    pub auth: AuthConfig,
    /// Alert-storm controls (notification throttle / aggregation).
    #[serde(default)]
    pub storm: StormConfig,
    /// Multi-instance leader election (Redis lease).
    #[serde(default)]
    pub cluster: ClusterConfig,
    /// Elasticsearch connection for optional alert history (toggles live in DB / UI).
    #[serde(default)]
    pub elasticsearch: ElasticsearchConfig,
    /// Reverse-proxy target for console `/trap-api/*` (Trap service HTTP).
    #[serde(default)]
    pub trap: TrapProxyConfig,
}

/// `[trap]` — Trap service BFF + MIB/policy storage (RustFS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapProxyConfig {
    /// e.g. `http://127.0.0.1:8081`. Empty disables `/trap-api` proxy.
    #[serde(default)]
    pub api_url: String,
    /// Shared with Trap `api_token`; injected as Bearer when proxying `/trap-api`.
    #[serde(default)]
    pub api_token: String,
    /// RustFS / S3 for MIB object bodies (CRUD on this server).
    #[serde(default)]
    pub s3_endpoint: String,
    #[serde(default)]
    pub s3_access_key: String,
    #[serde(default)]
    pub s3_secret_key: String,
    #[serde(default = "default_s3_bucket")]
    pub s3_bucket: String,
    #[serde(default = "default_s3_region")]
    pub s3_region: String,
    /// Local cache dir for mib-rs (OID browser / export).
    #[serde(default = "default_mib_cache")]
    pub mib_cache_dir: String,
}

fn default_s3_bucket() -> String {
    "eventide-mibs".into()
}
fn default_s3_region() -> String {
    "us-east-1".into()
}
fn default_mib_cache() -> String {
    "data/mib-cache".into()
}

impl Default for TrapProxyConfig {
    fn default() -> Self {
        Self {
            api_url: String::new(),
            api_token: String::new(),
            s3_endpoint: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_bucket: default_s3_bucket(),
            s3_region: default_s3_region(),
            mib_cache_dir: default_mib_cache(),
        }
    }
}

impl TrapProxyConfig {
    pub fn s3_configured(&self) -> bool {
        !self.s3_endpoint.trim().is_empty()
            && !self.s3_access_key.is_empty()
            && !self.s3_secret_key.is_empty()
    }
}

/// `[elasticsearch]` — connection only; enable write / search store via console settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElasticsearchConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_es_index")]
    pub index: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

fn default_es_index() -> String {
    "eventide-alerts".into()
}

impl Default for ElasticsearchConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            index: default_es_index(),
            username: String::new(),
            password: String::new(),
        }
    }
}

/// `[storm]` — throttle (P0) + aggregation (P1). See README §13.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StormConfig {
    #[serde(default = "default_true")]
    pub throttle_enabled: bool,
    #[serde(default = "default_min_interval")]
    pub min_interval_seconds: u64,
    #[serde(default = "default_max_per_window")]
    pub max_per_window: u32,
    #[serde(default = "default_window_seconds")]
    pub window_seconds: u64,
    /// `fingerprint` or `labels:alertname,ip`
    #[serde(default = "default_throttle_key")]
    pub throttle_key: String,
    /// BecameResolved ceiling; defaults to `max_per_window` when omitted.
    #[serde(default)]
    pub resolve_max_per_window: Option<u32>,
    // P1 aggregation
    #[serde(default)]
    pub aggregate_enabled: bool,
    #[serde(default = "default_agg_window")]
    pub aggregate_window_seconds: u64,
    #[serde(default = "default_group_by")]
    pub group_by: String,
    #[serde(default = "default_agg_mode")]
    pub aggregate_mode: String,
    #[serde(default = "default_agg_samples")]
    pub aggregate_sample_labels: String,
    #[serde(default = "default_agg_sample_limit")]
    pub aggregate_sample_limit: usize,
    // P2 ingress pressure
    #[serde(default = "default_ingress_max_inflight")]
    pub ingress_max_inflight: usize,
    #[serde(default)]
    pub degrade_skip_notify: bool,
    #[serde(default = "default_degrade_notify_per_sec")]
    pub degrade_notify_per_sec: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_user")]
    pub username: String,
    #[serde(default = "default_password")]
    pub password: String,
    /// HMAC secret for JWT. Change in production.
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// Token TTL in hours.
    #[serde(default = "default_token_ttl")]
    pub token_ttl_hours: u64,
    /// Max failed logins per IP / username within `login_window_seconds` before lockout.
    #[serde(default = "default_login_max_failures")]
    pub login_max_failures: u32,
    /// Sliding window for counting failures (seconds).
    #[serde(default = "default_login_window_secs")]
    pub login_window_seconds: u64,
    /// Lockout duration after threshold (seconds).
    #[serde(default = "default_login_lockout_secs")]
    pub login_lockout_seconds: u64,
    /// Password max age in days (0 = disable expiry reminders).
    #[serde(default = "default_password_max_age_days")]
    pub password_max_age_days: u64,
    /// Warn when remaining days ≤ this (and policy enabled).
    #[serde(default = "default_password_warn_days")]
    pub password_warn_days: u64,
}

fn default_listen() -> String {
    "0.0.0.0:8080".into()
}
fn default_mysql() -> String {
    "mysql://eventide:eventide@127.0.0.1:3306/eventide".into()
}
fn default_redis() -> String {
    "redis://127.0.0.1:6379/".into()
}
fn default_static() -> String {
    "static".into()
}
fn default_v2_static() -> Option<String> {
    Some("console-vue/dist".to_string())
}
fn default_tick() -> u64 {
    5
}
fn default_user() -> String {
    "admin".into()
}
fn default_password() -> String {
    "admin123".into()
}
fn default_jwt_secret() -> String {
    "eventide-dev-secret-change-me".into()
}
fn default_token_ttl() -> u64 {
    24
}
fn default_login_max_failures() -> u32 {
    5
}
fn default_login_window_secs() -> u64 {
    900
}
fn default_login_lockout_secs() -> u64 {
    300
}
fn default_password_max_age_days() -> u64 {
    90
}
fn default_password_warn_days() -> u64 {
    14
}
fn default_true() -> bool {
    true
}
fn default_min_interval() -> u64 {
    60
}
fn default_max_per_window() -> u32 {
    20
}
fn default_window_seconds() -> u64 {
    60
}
fn default_throttle_key() -> String {
    "fingerprint".into()
}
fn default_agg_window() -> u64 {
    30
}
fn default_group_by() -> String {
    "alertname".into()
}
fn default_agg_mode() -> String {
    "head+summary".into()
}
fn default_agg_samples() -> String {
    "ip,instance,alertIp".into()
}
fn default_agg_sample_limit() -> usize {
    10
}
fn default_ingress_max_inflight() -> usize {
    100
}
fn default_degrade_notify_per_sec() -> u32 {
    50
}

impl Default for StormConfig {
    fn default() -> Self {
        Self {
            throttle_enabled: default_true(),
            min_interval_seconds: default_min_interval(),
            max_per_window: default_max_per_window(),
            window_seconds: default_window_seconds(),
            throttle_key: default_throttle_key(),
            resolve_max_per_window: None,
            aggregate_enabled: false,
            aggregate_window_seconds: default_agg_window(),
            group_by: default_group_by(),
            aggregate_mode: default_agg_mode(),
            aggregate_sample_labels: default_agg_samples(),
            aggregate_sample_limit: default_agg_sample_limit(),
            ingress_max_inflight: default_ingress_max_inflight(),
            degrade_skip_notify: false,
            degrade_notify_per_sec: default_degrade_notify_per_sec(),
        }
    }
}

impl StormConfig {
    /// Normalize strings / clamp zeros before persist or apply.
    pub fn normalize(mut self) -> Self {
        self.throttle_key = self.throttle_key.trim().to_string();
        if self.throttle_key.is_empty() {
            self.throttle_key = default_throttle_key();
        }
        self.group_by = self.group_by.trim().to_string();
        if self.group_by.is_empty() {
            self.group_by = default_group_by();
        }
        let mode = self.aggregate_mode.trim().to_ascii_lowercase();
        self.aggregate_mode = match mode.as_str() {
            "summary_only" | "summary-only" => "summary_only".into(),
            _ => "head+summary".into(),
        };
        self.aggregate_sample_labels = self
            .aggregate_sample_labels
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        if self.min_interval_seconds == 0 {
            self.min_interval_seconds = default_min_interval();
        }
        if self.max_per_window == 0 {
            self.max_per_window = default_max_per_window();
        }
        if self.window_seconds == 0 {
            self.window_seconds = default_window_seconds();
        }
        if self.aggregate_window_seconds == 0 {
            self.aggregate_window_seconds = default_agg_window();
        }
        if self.aggregate_sample_limit == 0 {
            self.aggregate_sample_limit = default_agg_sample_limit();
        }
        if self.degrade_notify_per_sec == 0 {
            self.degrade_notify_per_sec = default_degrade_notify_per_sec();
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        let key = self.throttle_key.trim();
        if key.is_empty() {
            return Err("throttle_key 不能为空".into());
        }
        if !(key == "fingerprint" || key.starts_with("labels:")) {
            return Err("throttle_key 须为 fingerprint 或 labels:字段[,字段…]".into());
        }
        if self.group_by.trim().is_empty() {
            return Err("group_by 不能为空".into());
        }
        let mode = self.aggregate_mode.trim().to_ascii_lowercase();
        if !matches!(
            mode.as_str(),
            "head+summary" | "head_summary" | "summary_only" | "summary-only"
        ) {
            return Err("aggregate_mode 须为 head+summary 或 summary_only".into());
        }
        Ok(())
    }

    pub fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig {
            enabled: self.throttle_enabled,
            min_interval_seconds: self.min_interval_seconds,
            max_per_window: self.max_per_window,
            window_seconds: self.window_seconds,
            resolve_max_per_window: self
                .resolve_max_per_window
                .unwrap_or(self.max_per_window),
        }
    }

    pub fn aggregate_config(&self) -> AggregateConfig {
        AggregateConfig {
            enabled: self.aggregate_enabled,
            window_seconds: self.aggregate_window_seconds,
            group_by: self.group_by.clone(),
            mode: AggregateMode::parse(&self.aggregate_mode),
            sample_labels: self
                .aggregate_sample_labels
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            sample_limit: self.aggregate_sample_limit.max(1),
        }
    }

    pub fn pressure_config(&self) -> IngressPressureConfig {
        IngressPressureConfig {
            max_inflight: self.ingress_max_inflight,
            degrade_skip_notify: self.degrade_skip_notify,
            degrade_notify_per_sec: self.degrade_notify_per_sec.max(1),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            username: default_user(),
            password: default_password(),
            jwt_secret: default_jwt_secret(),
            token_ttl_hours: default_token_ttl(),
            login_max_failures: default_login_max_failures(),
            login_window_seconds: default_login_window_secs(),
            login_lockout_seconds: default_login_lockout_secs(),
            password_max_age_days: default_password_max_age_days(),
            password_warn_days: default_password_warn_days(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            mysql_url: default_mysql(),
            redis_url: default_redis(),
            static_dir: default_static(),
            v2_static_dir: default_v2_static(),
            scheduler_tick_seconds: default_tick(),
            auth: AuthConfig::default(),
            storm: StormConfig::default(),
            cluster: ClusterConfig::default(),
            elasticsearch: ElasticsearchConfig::default(),
            trap: TrapProxyConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(toml::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("config {path} not found, using defaults");
                Ok(Self::default())
            }
            Err(e) => Err(e.into()),
        }
    }
}
