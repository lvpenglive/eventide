//! Shared application state.

use crate::alert_history::AlertHistoryPrefs;
use crate::config::AppConfig;
use crate::db::Db;
use crate::elasticsearch::EsClient;
use crate::leader::LeaderElection;
use crate::notify_pipeline::NotifyQueue;
use eventide_core::{IngressPressure, RedisAggregateBuffer, RedisThrottleGate};
use eventide_notify::Notifier;
use eventide_sources::KafkaClientPool;
use eventide_trap_data::{MibStore, PolicyRedis, PolicyStore};
use std::sync::{Arc, RwLock};

pub struct AppState {
    pub db: Db,
    pub config: AppConfig,
    pub notifier: Notifier,
    pub throttle: RedisThrottleGate,
    pub aggregate: RedisAggregateBuffer,
    pub pressure: IngressPressure,
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
}

impl AppState {
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
