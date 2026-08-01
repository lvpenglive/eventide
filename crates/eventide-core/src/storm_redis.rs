//! Redis-backed alert-storm throttle + aggregation (multi-instance).

use crate::models::AlertTransition;
use crate::storm::{
    AggregateConfig, AggregateMode, AggregatePushResult, AggregateSummary, ThrottleConfig,
    ThrottleDecision,
};
use chrono::{DateTime, Utc};
use redis::Commands;
use std::sync::Mutex;

const THROTTLE_LUA: &str = r#"
local key = KEYS[1]
local now = tonumber(ARGV[1])
local min_gap = tonumber(ARGV[2])
local window = tonumber(ARGV[3])
local maxc = tonumber(ARGV[4])
local last = tonumber(redis.call('HGET', key, 'last') or '0')
local ws = tonumber(redis.call('HGET', key, 'ws') or '0')
local count = tonumber(redis.call('HGET', key, 'count') or '0')
if last > 0 and now < last + min_gap then
  return 1
end
if ws == 0 or now >= ws + window then
  ws = now
  count = 0
end
if count >= maxc then
  return 2
end
count = count + 1
redis.call('HSET', key, 'last', now, 'ws', ws, 'count', count)
redis.call('PEXPIRE', key, math.max(window, min_gap) * 2000)
return 0
"#;

/// Redis-backed throttle gate (shared across instances).
pub struct RedisThrottleGate {
    config: Mutex<ThrottleConfig>,
    conn: Mutex<redis::Connection>,
    prefix: String,
}

impl RedisThrottleGate {
    pub fn new(config: ThrottleConfig, client: &redis::Client) -> redis::RedisResult<Self> {
        Ok(Self {
            config: Mutex::new(config),
            conn: Mutex::new(client.get_connection()?),
            prefix: "eventide:throttle:".into(),
        })
    }

    pub fn config(&self) -> ThrottleConfig {
        self.config
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn set_config(&self, config: ThrottleConfig) {
        if let Ok(mut g) = self.config.lock() {
            *g = config;
        }
    }

    pub fn allow(
        &self,
        channel_id: &str,
        throttle_key: &str,
        transition: AlertTransition,
        now: DateTime<Utc>,
    ) -> ThrottleDecision {
        let cfg = self.config();
        if !cfg.enabled {
            return ThrottleDecision::Disabled;
        }
        if !matches!(
            transition,
            AlertTransition::BecameFiring | AlertTransition::BecameResolved
        ) {
            return ThrottleDecision::Allow;
        }
        let side = match transition {
            AlertTransition::BecameResolved => "resolved",
            _ => "firing",
        };
        let max = match transition {
            AlertTransition::BecameResolved => cfg.resolve_max_per_window,
            _ => cfg.max_per_window,
        };
        let key = format!("{}{}|{}|{}", self.prefix, channel_id, side, throttle_key);
        let now_ms = now.timestamp_millis();
        let min_gap = (cfg.min_interval_seconds as i64) * 1000;
        let window = (cfg.window_seconds as i64) * 1000;

        let mut conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return ThrottleDecision::Allow,
        };
        let result: Result<i32, _> = redis::Script::new(THROTTLE_LUA)
            .key(&key)
            .arg(now_ms)
            .arg(min_gap)
            .arg(window)
            .arg(max)
            .invoke(&mut *conn);
        match result {
            Ok(0) => ThrottleDecision::Allow,
            Ok(1) => ThrottleDecision::DenyMinInterval,
            Ok(2) => ThrottleDecision::DenyWindowLimit,
            _ => ThrottleDecision::Allow,
        }
    }
}

/// Redis-backed aggregation buffer.
pub struct RedisAggregateBuffer {
    config: Mutex<AggregateConfig>,
    conn: Mutex<redis::Connection>,
    prefix: String,
}

impl RedisAggregateBuffer {
    pub fn new(config: AggregateConfig, client: &redis::Client) -> redis::RedisResult<Self> {
        Ok(Self {
            config: Mutex::new(config),
            conn: Mutex::new(client.get_connection()?),
            prefix: "eventide:agg:".into(),
        })
    }

    pub fn config(&self) -> AggregateConfig {
        self.config
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn set_config(&self, config: AggregateConfig) {
        if let Ok(mut g) = self.config.lock() {
            *g = config;
        }
    }

    pub fn enabled(&self) -> bool {
        self.config().enabled
    }

    fn bucket_key(&self, channel_id: &str, group_key: &str) -> String {
        format!("{}bucket:{}|{}", self.prefix, channel_id, group_key)
    }

    fn index_key(&self) -> String {
        format!("{}index", self.prefix)
    }

    pub fn push(
        &self,
        channel_id: &str,
        group_key: &str,
        fingerprint: &str,
        sample: impl Into<String>,
        now: DateTime<Utc>,
    ) -> AggregatePushResult {
        let cfg = self.config();
        if !cfg.enabled {
            return AggregatePushResult::Bypass;
        }
        let sample = sample.into();
        let window_ms = (cfg.window_seconds.max(1) as i64) * 1000;
        let limit = cfg.sample_limit.max(1);
        let mode_head = matches!(cfg.mode, AggregateMode::HeadSummary);
        let member = format!("{channel_id}|{group_key}");
        let bkey = self.bucket_key(channel_id, group_key);
        let idx = self.index_key();
        let now_ms = now.timestamp_millis();

        let mut conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return AggregatePushResult::Bypass,
        };

        let ws: i64 = conn.hget(&bkey, "ws").unwrap_or(0);
        let in_window = ws > 0 && now_ms < ws + window_ms;

        if in_window {
            let _: Result<(), _> = conn.hincr(&bkey, "count", 1i64);
            self.append_sample(&mut conn, &bkey, &sample, fingerprint, limit);
            let _: Result<(), _> = conn.pexpire(&bkey, window_ms * 2);
            return AggregatePushResult::Buffered;
        }

        let head = mode_head;
        let samples_json = if sample.is_empty() {
            "[]".to_string()
        } else {
            serde_json::to_string(&vec![&sample]).unwrap_or_else(|_| "[]".into())
        };
        let fps_json = serde_json::to_string(&vec![fingerprint]).unwrap_or_else(|_| "[]".into());
        let _: Result<(), _> = conn.hset_multiple(
            &bkey,
            &[
                ("ws", now_ms.to_string()),
                ("count", "1".into()),
                ("head", if head { "1" } else { "0" }.into()),
                ("samples", samples_json),
                ("fps", fps_json),
            ],
        );
        let _: Result<(), _> = conn.zadd(&idx, &member, (now_ms + window_ms) as f64);
        let _: Result<(), _> = conn.pexpire(&bkey, window_ms * 2);

        if head {
            AggregatePushResult::SendHead
        } else {
            AggregatePushResult::Buffered
        }
    }

    fn append_sample(
        &self,
        conn: &mut redis::Connection,
        bkey: &str,
        sample: &str,
        fingerprint: &str,
        limit: usize,
    ) {
        let samples_raw: String = conn.hget(bkey, "samples").unwrap_or_else(|_| "[]".into());
        let mut samples: Vec<String> = serde_json::from_str(&samples_raw).unwrap_or_default();
        if samples.len() < limit && !sample.is_empty() {
            samples.push(sample.to_string());
            let _: Result<(), _> = conn.hset(
                bkey,
                "samples",
                serde_json::to_string(&samples).unwrap_or_default(),
            );
        }
        let fps_raw: String = conn.hget(bkey, "fps").unwrap_or_else(|_| "[]".into());
        let mut fps: Vec<String> = serde_json::from_str(&fps_raw).unwrap_or_default();
        if fps.len() < limit {
            fps.push(fingerprint.to_string());
            let _: Result<(), _> =
                conn.hset(bkey, "fps", serde_json::to_string(&fps).unwrap_or_default());
        }
    }

    pub fn take_due(&self, now: DateTime<Utc>) -> Vec<AggregateSummary> {
        let cfg = self.config();
        if !cfg.enabled {
            return Vec::new();
        }
        let now_ms = now.timestamp_millis() as f64;
        let idx = self.index_key();
        let mut conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let members: Vec<String> = conn.zrangebyscore(&idx, 0.0, now_ms).unwrap_or_default();
        let mut out = Vec::new();
        for member in members {
            let _: Result<(), _> = conn.zrem(&idx, &member);
            let Some((channel_id, group_key)) = member.split_once('|') else {
                continue;
            };
            let bkey = self.bucket_key(channel_id, group_key);
            let ws: i64 = conn.hget(&bkey, "ws").unwrap_or(0);
            let count: i64 = conn.hget(&bkey, "count").unwrap_or(0);
            let head: i64 = conn.hget(&bkey, "head").unwrap_or(0);
            let samples_raw: String = conn.hget(&bkey, "samples").unwrap_or_else(|_| "[]".into());
            let samples: Vec<String> = serde_json::from_str(&samples_raw).unwrap_or_default();
            let _: Result<(), _> = conn.del(&bkey);

            if matches!(cfg.mode, AggregateMode::HeadSummary) && head == 1 && count <= 1 {
                continue;
            }
            if count == 0 {
                continue;
            }
            let window_start = DateTime::from_timestamp_millis(ws).unwrap_or(now);
            out.push(AggregateSummary {
                channel_id: channel_id.to_string(),
                group_key: group_key.to_string(),
                count: count as u32,
                samples,
                window_start,
                window_end: now,
            });
        }
        out
    }
}
