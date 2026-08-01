//! Redis snapshot + pub/sub for Trap policy distribution.
//!
//! MySQL remains the source of truth. Eventide server writes MySQL then calls
//! [`PolicyRedis::publish_snapshot`]. Trap instances load from Redis and
//! subscribe to [`POLICY_CHANGED_CHANNEL`] for near-realtime reload.

use crate::policy_store::TrapPolicy;
use anyhow::{Context, Result};
use redis::Commands;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Redis key holding the full policy snapshot JSON.
pub const POLICY_SNAPSHOT_KEY: &str = "eventide:trap:policies:snapshot";
/// Pub/Sub channel — payload is the new stamp (or `"changed"`).
pub const POLICY_CHANGED_CHANNEL: &str = "eventide:trap:policies:changed";

/// Redis key for Trap HTTP `api_token` (shared with eventide-server proxy).
pub const TRAP_API_TOKEN_KEY: &str = "eventide:trap:api_token";
/// Pub/Sub when Trap HTTP token is rotated from the console.
pub const TRAP_API_TOKEN_CHANNEL: &str = "eventide:trap:api_token:changed";

/// Redis key prefix for Trap instance heartbeats: `eventide:trap:instances:{id}`.
pub const TRAP_INSTANCE_KEY_PREFIX: &str = "eventide:trap:instances:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapInstanceHeartbeat {
    pub instance_id: String,
    pub listen_udp: String,
    pub listen_http: String,
    #[serde(default)]
    pub ha_vip: String,
    #[serde(default)]
    pub kafka_topic: String,
    #[serde(default)]
    pub received: u64,
    #[serde(default)]
    pub parsed_ok: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySnapshot {
    /// Usually MAX(updated_at); used by Trap to skip no-op reloads.
    pub stamp: String,
    pub policies: Vec<TrapPolicy>,
}

/// Thin Redis helper shared by eventide-server (publish) and eventide-trap (load/subscribe).
pub struct PolicyRedis {
    client: redis::Client,
    conn: Mutex<redis::Connection>,
}

impl PolicyRedis {
    pub fn connect(redis_url: &str) -> Result<Self> {
        if redis_url.trim().is_empty() {
            anyhow::bail!("redis_url is empty");
        }
        let client = redis::Client::open(redis_url).context("parse redis_url")?;
        let conn = client
            .get_connection_with_timeout(std::time::Duration::from_secs(5))
            .context("redis connect")?;
        let _ = conn.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let _ = conn.set_write_timeout(Some(std::time::Duration::from_secs(5)));
        Ok(Self {
            client,
            conn: Mutex::new(conn),
        })
    }

    fn with_conn<T>(&self, mut f: impl FnMut(&mut redis::Connection) -> Result<T>) -> Result<T> {
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("redis mutex poisoned"))?;
        match f(&mut guard) {
            Ok(v) => Ok(v),
            Err(e) => {
                // Stale / half-open TCP — reconnect once then retry.
                tracing::debug!(error = %e, "redis command failed; reconnecting");
                let fresh = self
                    .client
                    .get_connection_with_timeout(std::time::Duration::from_secs(5))
                    .context("redis reconnect")?;
                let _ = fresh.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let _ = fresh.set_write_timeout(Some(std::time::Duration::from_secs(5)));
                *guard = fresh;
                f(&mut guard)
            }
        }
    }

    pub fn client(&self) -> &redis::Client {
        &self.client
    }

    /// SET snapshot + PUBLISH change notification.
    pub fn publish_snapshot(&self, policies: &[TrapPolicy]) -> Result<String> {
        let stamp = policies
            .iter()
            .map(|p| p.updated_at.as_str())
            .max()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let snap = PolicySnapshot {
            stamp: stamp.clone(),
            policies: policies.to_vec(),
        };
        let raw = serde_json::to_string(&snap).context("serialize policy snapshot")?;
        self.with_conn(|conn| {
            let _: () = conn
                .set(POLICY_SNAPSHOT_KEY, &raw)
                .context("redis SET policy snapshot")?;
            let _: i64 = conn
                .publish(POLICY_CHANGED_CHANNEL, &stamp)
                .context("redis PUBLISH policy changed")?;
            Ok(())
        })?;
        tracing::info!(
            stamp = %stamp,
            count = policies.len(),
            "published trap policy snapshot to redis"
        );
        Ok(stamp)
    }

    pub fn load_snapshot(&self) -> Result<Option<PolicySnapshot>> {
        let raw: Option<String> = self.with_conn(|conn| {
            conn.get(POLICY_SNAPSHOT_KEY)
                .context("redis GET policy snapshot")
        })?;
        let Some(raw) = raw.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let snap: PolicySnapshot =
            serde_json::from_str(&raw).context("parse policy snapshot JSON")?;
        Ok(Some(snap))
    }

    pub fn snapshot_stamp(&self) -> Result<Option<String>> {
        Ok(self.load_snapshot()?.map(|s| s.stamp))
    }

    /// SET Trap HTTP api_token + PUBLISH (empty string clears / disables auth).
    pub fn publish_api_token(&self, token: &str) -> Result<()> {
        self.with_conn(|conn| {
            let _: () = conn
                .set(TRAP_API_TOKEN_KEY, token)
                .context("redis SET trap api_token")?;
            let _: i64 = conn
                .publish(TRAP_API_TOKEN_CHANNEL, if token.is_empty() { "cleared" } else { "changed" })
                .context("redis PUBLISH trap api_token")?;
            Ok(())
        })?;
        tracing::info!(
            set = !token.is_empty(),
            "published trap api_token to redis"
        );
        Ok(())
    }

    pub fn load_api_token(&self) -> Result<Option<String>> {
        let raw: Option<String> = self.with_conn(|conn| {
            conn.get(TRAP_API_TOKEN_KEY)
                .context("redis GET trap api_token")
        })?;
        Ok(raw)
    }

    /// SET heartbeat with TTL (seconds). Keepalived VIP HA uses this for console visibility.
    pub fn beat_instance(&self, hb: &TrapInstanceHeartbeat, ttl_secs: u64) -> Result<()> {
        let key = format!("{TRAP_INSTANCE_KEY_PREFIX}{}", hb.instance_id);
        let raw = serde_json::to_string(hb).context("serialize trap instance heartbeat")?;
        let ttl = ttl_secs.max(3) as i64;
        self.with_conn(|conn| {
            let _: () = redis::cmd("SET")
                .arg(&key)
                .arg(&raw)
                .arg("EX")
                .arg(ttl)
                .query(conn)
                .context("redis SET trap instance heartbeat")?;
            Ok(())
        })
    }

    pub fn list_instances(&self) -> Result<Vec<TrapInstanceHeartbeat>> {
        let pattern = format!("{TRAP_INSTANCE_KEY_PREFIX}*");
        let keys: Vec<String> = self.with_conn(|conn| {
            conn.keys(&pattern).context("redis KEYS trap instances")
        })?;
        let mut out = Vec::with_capacity(keys.len());
        for key in keys {
            let raw: Option<String> = self.with_conn(|conn| {
                conn.get(&key).context("redis GET trap instance")
            })?;
            let Some(raw) = raw.filter(|s| !s.is_empty()) else {
                continue;
            };
            match serde_json::from_str::<TrapInstanceHeartbeat>(&raw) {
                Ok(hb) => out.push(hb),
                Err(e) => tracing::warn!(%key, error = %e, "skip bad trap instance heartbeat"),
            }
        }
        out.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
        Ok(out)
    }
}
