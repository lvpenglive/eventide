//! Application configuration (TOML).

use eventide_core::{AggregateConfig, AggregateMode, IngressPressureConfig, ThrottleConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_db")]
    pub database_path: String,
    #[serde(default = "default_static")]
    pub static_dir: String,
    /// Global scheduler tick in seconds (rules still have their own interval).
    #[serde(default = "default_tick")]
    pub scheduler_tick_seconds: u64,
    #[serde(default)]
    pub auth: AuthConfig,
    /// Alert-storm controls (notification throttle / future aggregation).
    #[serde(default)]
    pub storm: StormConfig,
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
}

fn default_listen() -> String {
    "0.0.0.0:8080".into()
}
fn default_db() -> String {
    "data/eventide.db".into()
}
fn default_static() -> String {
    "static".into()
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
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            database_path: default_db(),
            static_dir: default_static(),
            scheduler_tick_seconds: default_tick(),
            auth: AuthConfig::default(),
            storm: StormConfig::default(),
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
