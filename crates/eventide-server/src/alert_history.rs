//! Runtime prefs: ES connection + write toggle + default search store.

use crate::elasticsearch::{EsClient, EsConfig};
use serde::{Deserialize, Serialize};

pub const ALERT_HISTORY_KEY: &str = "alert_history";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlertHistoryPrefs {
    /// When true and URL is set, upserts are indexed to Elasticsearch.
    #[serde(default)]
    pub write_to_es: bool,
    /// Default list source: `mysql` | `es`.
    #[serde(default = "default_store")]
    pub search_store: String,
    /// Elasticsearch HTTP URL, editable from console (overrides empty toml).
    #[serde(default)]
    pub es_url: String,
    #[serde(default = "default_index")]
    pub es_index: String,
    #[serde(default)]
    pub es_username: String,
    #[serde(default)]
    pub es_password: String,
}

fn default_store() -> String {
    "mysql".into()
}

fn default_index() -> String {
    "eventide-alerts".into()
}

impl Default for AlertHistoryPrefs {
    fn default() -> Self {
        Self {
            write_to_es: false,
            search_store: default_store(),
            es_url: String::new(),
            es_index: default_index(),
            es_username: String::new(),
            es_password: String::new(),
        }
    }
}

impl AlertHistoryPrefs {
    pub fn normalize(mut self) -> Self {
        let s = self.search_store.trim().to_ascii_lowercase();
        self.search_store = if s == "es" || s == "elasticsearch" {
            "es".into()
        } else {
            "mysql".into()
        };
        self.es_url = self.es_url.trim().to_string();
        if self.es_index.trim().is_empty() {
            self.es_index = default_index();
        } else {
            self.es_index = self.es_index.trim().to_string();
        }
        self
    }

    pub fn es_configured(&self) -> bool {
        !self.es_url.is_empty()
    }

    /// Seed empty connection fields from toml defaults.
    pub fn with_toml_fallback(
        mut self,
        url: &str,
        index: &str,
        username: &str,
        password: &str,
    ) -> Self {
        if self.es_url.trim().is_empty() && !url.trim().is_empty() {
            self.es_url = url.trim().to_string();
        }
        if self.es_index.trim().is_empty() || self.es_index == default_index() {
            if !index.trim().is_empty() {
                self.es_index = index.trim().to_string();
            }
        }
        if self.es_username.is_empty() && !username.is_empty() {
            self.es_username = username.to_string();
        }
        if self.es_password.is_empty() && !password.is_empty() {
            self.es_password = password.to_string();
        }
        self.normalize()
    }

    pub fn build_es_client(&self) -> anyhow::Result<Option<EsClient>> {
        if !self.es_configured() {
            return Ok(None);
        }
        let cfg = EsConfig {
            url: self.es_url.clone(),
            index: self.es_index.clone(),
            username: if self.es_username.is_empty() {
                None
            } else {
                Some(self.es_username.clone())
            },
            password: if self.es_password.is_empty() {
                None
            } else {
                Some(self.es_password.clone())
            },
        };
        Ok(Some(EsClient::new(cfg)?))
    }
}
