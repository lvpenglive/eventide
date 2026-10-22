//! Redis registry of live eventide-server instances (console cluster view).
//!
//! Keys: `eventide:server:instances:{instance_id}` with TTL = 3× heartbeat interval.
//! Trap instances remain under `eventide:trap:instances:*` (PolicyRedis).

use crate::state::AppState;
use chrono::Utc;
use redis::Commands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

pub const SERVER_INSTANCE_KEY_PREFIX: &str = "eventide:server:instances:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInstanceHeartbeat {
    pub instance_id: String,
    pub listen: String,
    #[serde(default)]
    pub hostname: String,
    pub is_leader: bool,
    pub cluster_enabled: bool,
    #[serde(default)]
    pub version: String,
    pub started_at: String,
    pub updated_at: String,
}

fn local_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

pub fn beat_server(client: &redis::Client, hb: &ServerInstanceHeartbeat, ttl_secs: u64) -> redis::RedisResult<()> {
    let key = format!("{SERVER_INSTANCE_KEY_PREFIX}{}", hb.instance_id);
    let raw = serde_json::to_string(hb).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::IoError,
            "serialize server heartbeat",
            e.to_string(),
        ))
    })?;
    let mut conn = client.get_connection()?;
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg(&raw)
        .arg("EX")
        .arg(ttl_secs.max(3) as i64)
        .query(&mut conn)?;
    Ok(())
}

pub fn list_servers(client: &redis::Client) -> redis::RedisResult<Vec<ServerInstanceHeartbeat>> {
    let mut conn = client.get_connection()?;
    let pattern = format!("{SERVER_INSTANCE_KEY_PREFIX}*");
    let keys: Vec<String> = conn.keys(&pattern)?;
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let values: Vec<Option<String>> = redis::cmd("MGET").arg(&keys).query(&mut conn)?;
    let mut out = Vec::with_capacity(values.len());
    for (key, raw) in keys.into_iter().zip(values) {
        let Some(raw) = raw.filter(|s| !s.is_empty()) else {
            continue;
        };
        match serde_json::from_str::<ServerInstanceHeartbeat>(&raw) {
            Ok(hb) => out.push(hb),
            Err(e) => tracing::warn!(%key, error = %e, "skip bad server instance heartbeat"),
        }
    }
    out.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    Ok(out)
}

/// Snapshot for overview / `/api/cluster/instances`.
pub fn cluster_snapshot(state: &AppState) -> serde_json::Value {
    let servers = match list_servers(&state.redis) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "list server instances failed");
            Vec::new()
        }
    };
    let (traps, trap_redis) = match state.policy_redis.as_ref() {
        Some(redis) => match redis.list_instances() {
            Ok(v) => (v, true),
            Err(e) => {
                tracing::warn!(error = %e, "list trap instances failed");
                (Vec::new(), true)
            }
        },
        None => (Vec::new(), false),
    };
    serde_json::json!({
        "redis": true,
        "trap_redis": trap_redis,
        "servers": servers,
        "traps": traps,
        "server_count": servers.len(),
        "trap_count": traps.len(),
    })
}

/// Publish this process's presence every few seconds.
pub fn spawn_server_heartbeat(state: Arc<AppState>, redis: redis::Client, interval_secs: u64) {
    let interval_secs = interval_secs.max(3);
    let ttl = interval_secs.saturating_mul(3);
    let started_at = Utc::now().to_rfc3339();
    let instance_id = state.leader.holder_id().to_string();
    let listen = state.config.listen.clone();
    let hostname = local_hostname();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let cluster_enabled = state.config.cluster.enabled;

    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(interval_secs));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut fail_streak = 0u32;
        loop {
            tick.tick().await;
            let hb = ServerInstanceHeartbeat {
                instance_id: instance_id.clone(),
                listen: listen.clone(),
                hostname: hostname.clone(),
                is_leader: state.leader.is_leader(),
                cluster_enabled,
                version: version.clone(),
                started_at: started_at.clone(),
                updated_at: Utc::now().to_rfc3339(),
            };
            let redis = redis.clone();
            let result = tokio::task::spawn_blocking(move || beat_server(&redis, &hb, ttl)).await;
            match result {
                Ok(Ok(())) => fail_streak = 0,
                Ok(Err(e)) => {
                    fail_streak = fail_streak.saturating_add(1);
                    if fail_streak == 1 || fail_streak % 12 == 0 {
                        tracing::warn!(error = %e, fail_streak, "server instance heartbeat failed");
                    }
                }
                Err(e) => {
                    fail_streak = fail_streak.saturating_add(1);
                    tracing::warn!(error = %e, fail_streak, "server instance heartbeat join failed");
                }
            }
        }
    });
}
