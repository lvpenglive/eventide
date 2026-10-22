//! Redis lease-based leader election for cluster-safe background work.
//!
//! Only the holder of `eventide:cluster:leader` (or configured key) runs the
//! rule scheduler and aggregate flusher. Kafka ingress scales out via Kafka
//! consumer groups (no Redis partition leases).
//! All instances still serve HTTP API / ingress webhooks.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    client: redis::Client,
    is_leader: AtomicBool,
}

impl LeaderElection {
    pub fn new(client: &redis::Client, cfg: &ClusterConfig) -> redis::RedisResult<Self> {
        let holder_id = Uuid::new_v4().to_string();
        // Validate connectivity once at boot.
        let mut conn = client.get_connection()?;
        let _: String = redis::cmd("PING").query(&mut conn)?;
        Ok(Self {
            enabled: cfg.enabled,
            key: cfg.leader_key.clone(),
            holder_id,
            lease_seconds: cfg.lease_seconds.max(3),
            client: client.clone(),
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

    fn with_conn<T>(&self, f: impl FnOnce(&mut redis::Connection) -> redis::RedisResult<T>) -> redis::RedisResult<T> {
        let mut conn = self.client.get_connection()?;
        f(&mut conn)
    }

    /// Try to become leader or renew the lease if we already hold it.
    pub fn tick(&self) -> bool {
        if !self.enabled {
            self.is_leader.store(true, Ordering::Release);
            return true;
        }

        let was = self.is_leader.load(Ordering::Acquire);
        let became = match self.contend(was) {
            Ok(v) => v,
            Err(e) => {
                // Transient Redis errors must not demote a healthy leader, and must not
                // leave a follower stuck while our own lease key is still alive.
                tracing::warn!(error = %e, holder = %self.holder_id, key = %self.key, "leader election tick failed");
                return was;
            }
        };

        let prev = self.is_leader.swap(became, Ordering::AcqRel);
        if became && !prev {
            tracing::info!(holder = %self.holder_id, key = %self.key, "became cluster leader");
        } else if !became && prev {
            tracing::warn!(holder = %self.holder_id, key = %self.key, "lost cluster leadership");
        }
        became
    }

    fn contend(&self, currently_leader: bool) -> redis::RedisResult<bool> {
        self.with_conn(|conn| {
            if currently_leader {
                let ok: i32 = redis::Script::new(RENEW_LUA)
                    .key(&self.key)
                    .arg(&self.holder_id)
                    .arg(self.lease_seconds.to_string())
                    .invoke(conn)?;
                return Ok(ok == 1);
            }

            // Acquire, or reclaim if we still own the key (e.g. after a failed renew demotion).
            let acquired: Option<String> = redis::cmd("SET")
                .arg(&self.key)
                .arg(&self.holder_id)
                .arg("NX")
                .arg("EX")
                .arg(self.lease_seconds)
                .query(conn)?;
            if acquired.is_some() {
                return Ok(true);
            }

            let cur: Option<String> = redis::cmd("GET").arg(&self.key).query(conn)?;
            if cur.as_deref() == Some(self.holder_id.as_str()) {
                let ok: i32 = redis::Script::new(RENEW_LUA)
                    .key(&self.key)
                    .arg(&self.holder_id)
                    .arg(self.lease_seconds.to_string())
                    .invoke(conn)?;
                return Ok(ok == 1);
            }
            Ok(false)
        })
    }

    pub fn renew_interval(&self) -> Duration {
        let secs = (self.lease_seconds / 3).max(1);
        Duration::from_secs(secs)
    }
}

/// Background loop: renew / contend for leadership.
pub fn spawn_leader_loop(leader: Arc<LeaderElection>) {
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking({
            let leader = leader.clone();
            move || leader.tick()
        })
        .await;
        let mut interval = tokio::time::interval(leader.renew_interval());
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Consume the immediate first tick so we don't renew in the same instant as acquire.
        interval.tick().await;
        loop {
            interval.tick().await;
            let leader = leader.clone();
            let _ = tokio::task::spawn_blocking(move || leader.tick()).await;
        }
    });
}
