//! Trap HTTP api_token — MySQL `app_kv` + runtime + Redis sync.

use serde::{Deserialize, Serialize};

pub const TRAP_API_TOKEN_KEY: &str = "trap_api_token";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrapApiTokenPrefs {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub updated_at: String,
}

impl TrapApiTokenPrefs {
    pub fn from_token(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub fn mask_token(token: &str) -> String {
    let t = token.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.len() <= 8 {
        return format!("{}…", &t[..t.len().min(2)]);
    }
    format!("{}…{}", &t[..4], &t[t.len() - 4..])
}

pub fn generate_token() -> String {
    format!("evt_{}", uuid::Uuid::new_v4().simple())
}
