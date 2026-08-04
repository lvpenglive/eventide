//! Shared application state.

use crate::alert_history::AlertHistoryPrefs;
use crate::config::{AppConfig, StormConfig};
use crate::db::Db;
use crate::elasticsearch::EsClient;
use crate::leader::LeaderElection;
use crate::license::LicenseGate;
use crate::notify_pipeline::NotifyQueue;
use eventide_core::{IngressPressure, RedisAggregateBuffer, RedisThrottleGate};
use eventide_notify::Notifier;
use eventide_sources::KafkaClientPool;
use eventide_trap_data::{MibStore, PolicyRedis, PolicyStore};
use std::sync::{Arc, RwLock};

pub const STORM_CONFIG_KEY: &str = "storm_config";

pub struct AppState {
    pub db: Db,
    pub config: AppConfig,
    pub notifier: Notifier,
    pub throttle: RedisThrottleGate,
    pub aggregate: RedisAggregateBuffer,
    pub pressure: IngressPressure,
    /// Runtime `[storm]` prefs (console / app_kv); hot-applied to throttle/aggregate/pressure.
    pub storm: Arc<RwLock<StormConfig>>,
    pub leader: Arc<LeaderElection>,
    pub kafka_pool: Arc<KafkaClientPool>,
    pub notify_queue: NotifyQueue,
    /// Hot-swappable ES client (rebuilt when settings change).
    pub es: Arc<RwLock<Option<EsClient>>>,
    /// Runtime prefs, mirrored in MySQL `app_kv`.
    pub alert_history: Arc<RwLock<AlertHistoryPrefs>>,
    /// MIB library (S3 + MySQL). None if `[trap]` S3 not configured.
    pub mibs: Option<Arc<MibStore>>,
    /// Trap policies (MySQL). None if pool open failed.
    pub policies: Option<Arc<PolicyStore>>,
    /// Redis snapshot publisher for Trap policy distribution.
    pub policy_redis: Option<Arc<PolicyRedis>>,
    /// Runtime Trap HTTP api_token (MySQL `app_kv` / console); falls back to toml.
    pub trap_api_token: Arc<RwLock<String>>,
    /// Product license / trial gate.
    pub license: Arc<LicenseGate>,
}

impl AppState {
    /// Effective token for `/trap-api` proxy: runtime (console) > toml.
    pub fn effective_trap_api_token(&self) -> String {
        if let Ok(g) = self.trap_api_token.read() {
            let t = g.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        self.config.trap.api_token.trim().to_string()
    }

    pub fn set_trap_api_token(&self, token: String) {
        if let Ok(mut g) = self.trap_api_token.write() {
            *g = token;
        }
    }

    pub fn sync_trap_api_token_to_redis(&self) {
        let Some(redis) = self.policy_redis.as_ref() else {
            return;
        };
        let token = self.effective_trap_api_token();
        if let Err(e) = redis.publish_api_token(&token) {
            tracing::warn!(error = %e, "failed to publish trap api_token to redis");
        }
    }

    pub fn storm_prefs(&self) -> StormConfig {
        self.storm
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| self.config.storm.clone())
    }

    /// Hot-apply storm prefs to in-memory gates (throttle / aggregate / pressure).
    pub fn set_storm_prefs(&self, storm: StormConfig) {
        if let Ok(mut g) = self.storm.write() {
            *g = storm.clone();
        }
        self.throttle.set_config(storm.throttle_config());
        self.aggregate.set_config(storm.aggregate_config());
        self.pressure.set_config(storm.pressure_config());
    }

    pub fn alert_history_prefs(&self) -> AlertHistoryPrefs {
        self.alert_history
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn set_alert_history_prefs(&self, prefs: AlertHistoryPrefs) {
        if let Ok(mut g) = self.alert_history.write() {
            *g = prefs;
        }
    }

    pub fn es_client(&self) -> Option<EsClient> {
        self.es.read().ok().and_then(|g| g.clone())
    }

    pub fn set_es_client(&self, client: Option<EsClient>) {
        if let Ok(mut g) = self.es.write() {
            *g = client;
        }
    }

    /// Push MySQL policies → Redis snapshot + PUBLISH (no-op if Redis helper missing).
    pub async fn sync_policies_to_redis(&self) {
        let (Some(policies), Some(redis)) = (&self.policies, &self.policy_redis) else {
            return;
        };
        let list = policies.list().await;
        if let Err(e) = redis.publish_snapshot(&list) {
            tracing::warn!(error = %e, "failed to publish trap policies to redis");
        }
    }
}
