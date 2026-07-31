//! Trap service configuration (TOML).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapConfig {
    /// HTTP API listen address (health / simulate / stats).
    #[serde(default = "default_http")]
    pub listen_http: String,
    /// UDP Trap listen address. Prefer `0.0.0.0:1162` in dev (162 needs privilege).
    #[serde(default = "default_udp")]
    pub listen_udp: String,
    /// Kafka broker list, comma-separated. Empty = parse-only (no produce).
    #[serde(default)]
    pub kafka_brokers: String,
    #[serde(default = "default_topic")]
    pub kafka_topic: String,
    /// Produce across N partitions by hashing the Kafka key (peer IP). Must match
    /// topic partition count and Kafka Ingress `options.partitions`.
    #[serde(default = "default_kafka_partitions")]
    pub kafka_partitions: i32,
    /// Fixed partition when `kafka_partitions == 1` (legacy single-partition mode).
    #[serde(default)]
    pub kafka_partition: i32,
    /// If non-empty, only accept SNMPv1/v2c traps with this community.
    #[serde(default)]
    pub community: String,
    #[serde(default = "default_severity")]
    pub default_severity: String,
    /// Keep last N normalized alerts for console preview.
    #[serde(default = "default_recent")]
    pub recent_limit: usize,
    /// Legacy local MIB directory — used only as one-time migration source.
    #[serde(default = "default_mib_dir")]
    pub mib_dir: String,
    /// Legacy policies JSON — used only as one-time migration source.
    #[serde(default = "default_policies_path")]
    pub policies_path: String,
    /// Eventide MySQL (policies + MIB metadata). Required.
    #[serde(default)]
    pub mysql_url: String,
    /// RustFS / S3 endpoint (e.g. http://host:9000).
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
    /// Local cache of MIB objects for mib-rs.
    #[serde(default = "default_mib_cache_dir")]
    pub mib_cache_dir: String,
    /// Seconds between memory reloads (Redis preferred, MySQL fallback).
    #[serde(default = "default_reload_secs")]
    pub policy_reload_secs: u64,
    /// Same Redis as Eventide server — policy snapshot + pub/sub. Empty = MySQL-only.
    #[serde(default)]
    pub redis_url: String,
}

fn default_http() -> String {
    "0.0.0.0:8081".into()
}
fn default_udp() -> String {
    "0.0.0.0:1162".into()
}
fn default_topic() -> String {
    "eventide.snmptrap".into()
}
fn default_kafka_partitions() -> i32 {
    1
}
fn default_severity() -> String {
    "warning".into()
}
fn default_recent() -> usize {
    50
}
fn default_mib_dir() -> String {
    "data/mibs".into()
}
fn default_policies_path() -> String {
    "data/trap-policies.json".into()
}
fn default_s3_bucket() -> String {
    "eventide-mibs".into()
}
fn default_s3_region() -> String {
    "us-east-1".into()
}
fn default_mib_cache_dir() -> String {
    "data/mib-cache".into()
}
fn default_reload_secs() -> u64 {
    10
}

impl Default for TrapConfig {
    fn default() -> Self {
        Self {
            listen_http: default_http(),
            listen_udp: default_udp(),
            kafka_brokers: String::new(),
            kafka_topic: default_topic(),
            kafka_partitions: default_kafka_partitions(),
            kafka_partition: 0,
            community: String::new(),
            default_severity: default_severity(),
            recent_limit: default_recent(),
            mib_dir: default_mib_dir(),
            policies_path: default_policies_path(),
            mysql_url: String::new(),
            s3_endpoint: String::new(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            s3_bucket: default_s3_bucket(),
            s3_region: default_s3_region(),
            mib_cache_dir: default_mib_cache_dir(),
            policy_reload_secs: default_reload_secs(),
            redis_url: String::new(),
        }
    }
}

impl TrapConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read config `{path}`: {e}"))?;
        let cfg: Self = toml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parse config `{path}`: {e}"))?;
        Ok(cfg)
    }
}
