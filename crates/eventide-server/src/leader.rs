//! Redis lease-based leader election for cluster-safe background work.
//!
//! Only the holder of `eventide:cluster:leader` (or configured key) runs the
//! rule scheduler and aggregate flusher. Kafka ingress scales out via Kafka
//! consumer groups (no Redis partition leases).
//! All instances still serve HTTP API / ingress webhooks.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const RENEW_LUA: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
  redis.call('SET', KEYS[1], ARGV[1], 'EX', tonumber(ARGV[2]))
  return 1
end
return 0
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// When false, every instance acts as leader (single-node convenience).
    #[serde(default = "default_cluster_enabled")]
    pub enabled: bool,
    #[serde(default = "default_leader_key")]
    pub leader_key: String,
    #[serde(default = "default_lease_seconds")]
    pub lease_seconds: u64,
    /// Deprecated: ignored. Kafka Ingress uses consumer groups.
    #[serde(default = "default_ingress_lease_seconds")]
    pub ingress_partition_lease_seconds: u64,
    /// Deprecated: ignored. Kafka Ingress uses consumer groups.
    #[serde(default)]
    pub ingress_max_claim: u32,
}

fn default_cluster_enabled() -> bool {
    true
}
fn default_leader_key() -> String {
    "eventide:cluster:leader".into()
}
fn default_lease_seconds() -> u64 {
    15
}
fn default_ingress_lease_seconds() -> u64 {
    15
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            leader_key: "eventide:cluster:leader".into(),
            lease_seconds: 15,
            ingress_partition_lease_seconds: 15,
            ingress_max_claim: 0,
        }
    }
}

pub struct LeaderElection {
    enabled: bool,
    key: String,
    holder_id: String,
    lease_seconds: u64,
    conn: Mutex<redis::Connection>,
    is_leader: AtomicBool,
}

impl LeaderElection {
    pub fn new(client: &redis::Client, cfg: &ClusterConfig) -> redis::RedisResult<Self> {
        let holder_id = Uuid::new_v4().to_string();
        Ok(Self {
            enabled: cfg.enabled,
            key: cfg.leader_key.clone(),
            holder_id,
            lease_seconds: cfg.lease_seconds.max(3),
            conn: Mutex::new(client.get_connection()?),
            is_leader: AtomicBool::new(!cfg.enabled),
        })
    }

    pub fn holder_id(&self) -> &str {
        &self.holder_id
    }

    pub fn is_leader(&self) -> bool {
        if !self.enabled {
            return true;
        }
        self.is_leader.load(Ordering::Acquire)
    }

    /// Try to become leader or renew the lease if we already hold it.
    pub fn tick(&self) -> bool {
        if !self.enabled {
            self.is_leader.store(true, Ordering::Release);
            return true;
        }
        let mut conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return self.is_leader(),
        };

        let became = if self.is_leader.load(Ordering::Acquire) {
            let ok: i32 = redis::Script::new(RENEW_LUA)
                .key(&self.key)
                .arg(&self.holder_id)
                .arg(self.lease_seconds)
                .invoke(&mut *conn)
                .unwrap_or(0);
            ok == 1
        } else {
            let r: Result<Option<String>, _> = redis::cmd("SET")
                .arg(&self.key)
                .arg(&self.holder_id)
                .arg("NX")
                .arg("EX")
                .arg(self.lease_seconds)
                .query(&mut *conn);
            matches!(r, Ok(Some(_)))
        };

        let was = self.is_leader.swap(became, Ordering::AcqRel);
        if became && !was {
            tracing::info!(holder = %self.holder_id, key = %self.key, "became cluster leader");
        } else if !became && was {
            tracing::warn!(holder = %self.holder_id, key = %self.key, "lost cluster leadership");
        }
        became
    }

    pub fn renew_interval(&self) -> Duration {
        let secs = (self.lease_seconds / 3).max(1);
        Duration::from_secs(secs)
    }
}

/// Background loop: renew / contend for leadership.
pub fn spawn_leader_loop(leader: Arc<LeaderElection>) {
    tokio::spawn(async move {
        leader.tick();
        let mut interval = tokio::time::interval(leader.renew_interval());
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let leader = leader.clone();
            let _ = tokio::task::spawn_blocking(move || leader.tick()).await;
        }
    });
}
