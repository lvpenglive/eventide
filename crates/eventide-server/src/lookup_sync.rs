//! Lookup sync token — MeridianOps / external systems push rows without user JWT.

use serde::{Deserialize, Serialize};

pub const LOOKUP_SYNC_KEY: &str = "lookup_sync";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LookupSyncPrefs {
    #[serde(default)]
    pub token: String,
    /// When non-empty, sync-token writes are limited to these lookup UUIDs.
    /// When empty, sync token may only write tables with `external_sync=true`.
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub updated_at: String,
}

impl LookupSyncPrefs {
    pub fn normalized_allowlist(&self) -> Vec<String> {
        self.allowlist
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

pub fn mask_token(token: &str) -> String {
    crate::trap_token::mask_token(token)
}

pub fn generate_token() -> String {
    format!("lks_{}", uuid::Uuid::new_v4().simple())
}
