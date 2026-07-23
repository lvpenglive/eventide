//! Application configuration (TOML).

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
