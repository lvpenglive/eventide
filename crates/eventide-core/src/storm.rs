//! Alert-storm notification throttle (P0).
//!
//! Limits how often the same `(channel, key, edge)` may actually send,
//! while callers still persist alert state.

use crate::models::{AlertTransition, Labels};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

/// Pure throttle parameters (loaded from `[storm]` on the server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrottleConfig {
    pub enabled: bool,
    pub min_interval_seconds: u64,
    pub max_per_window: u32,
    pub window_seconds: u64,
    /// Separate ceiling for BecameResolved (defaults equal to max_per_window).
    pub resolve_max_per_window: u32,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_interval_seconds: 60,
            max_per_window: 20,
            window_seconds: 60,
            resolve_max_per_window: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleDecision {
    Allow,
    Disabled,
    DenyMinInterval,
    DenyWindowLimit,
}

#[derive(Debug, Clone)]
struct Slot {
    last_sent_at: DateTime<Utc>,
    window_start: DateTime<Utc>,
    count: u32,
}

/// In-process gate. Restart clears state (acceptable for P0).
#[derive(Debug)]
pub struct ThrottleGate {
    config: ThrottleConfig,
    /// `channel|firing|key` or `channel|resolved|key`
    slots: Mutex<HashMap<String, Slot>>,
}

impl ThrottleGate {
    pub fn new(config: ThrottleConfig) -> Self {
        Self {
            config,
            slots: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &ThrottleConfig {
        &self.config
    }

    /// Returns whether this notification may call `notifier.send`.
    pub fn allow(
        &self,
        channel_id: &str,
        throttle_key: &str,
        transition: AlertTransition,
        now: DateTime<Utc>,
    ) -> ThrottleDecision {
        if !self.config.enabled {
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
            AlertTransition::BecameResolved => self.config.resolve_max_per_window,
            _ => self.config.max_per_window,
        };
        let map_key = format!("{channel_id}|{side}|{throttle_key}");
        let min_gap = Duration::seconds(self.config.min_interval_seconds as i64);
        let window = Duration::seconds(self.config.window_seconds as i64);

        let mut slots = self.slots.lock().expect("throttle mutex");
        let slot = slots.get(&map_key).cloned();

        if let Some(s) = slot {
            if now < s.last_sent_at + min_gap {
                return ThrottleDecision::DenyMinInterval;
            }
            let (window_start, count) = if now >= s.window_start + window {
                (now, 0)
            } else {
                (s.window_start, s.count)
            };
            if count >= max {
                return ThrottleDecision::DenyWindowLimit;
            }
            slots.insert(
                map_key,
                Slot {
                    last_sent_at: now,
                    window_start,
                    count: count + 1,
                },
            );
            ThrottleDecision::Allow
        } else {
            slots.insert(
                map_key,
                Slot {
                    last_sent_at: now,
                    window_start: now,
                    count: 1,
                },
            );
            ThrottleDecision::Allow
        }
    }
}

/// Build throttle key from config spec.
///
/// - `fingerprint` (default): use alert fingerprint  
/// - `labels:alertname,ip`: join listed label values with `|`
pub fn build_throttle_key(spec: &str, fingerprint: &str, labels: &Labels) -> String {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("fingerprint") {
        return fingerprint.to_string();
    }
    if let Some(rest) = spec
        .strip_prefix("labels:")
        .or_else(|| spec.strip_prefix("Labels:"))
    {
        let parts: Vec<String> = rest
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|k| labels.get(k).cloned().unwrap_or_default())
            .collect();
        let joined = parts.join("|");
        if joined.chars().all(|c| c == '|') || joined.is_empty() {
            return fingerprint.to_string();
        }
        return joined;
    }
    fingerprint.to_string()
}

// --- P1: time-window aggregation (notification only) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateMode {
    /// First alert in the window notifies immediately; window end sends a summary if count > 1.
    HeadSummary,
    /// Only a summary at window end.
    SummaryOnly,
}

impl AggregateMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "summary_only" | "summary-only" => Self::SummaryOnly,
            _ => Self::HeadSummary,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AggregateConfig {
    pub enabled: bool,
    pub window_seconds: u64,
    pub group_by: String,
    pub mode: AggregateMode,
    pub sample_labels: Vec<String>,
    pub sample_limit: usize,
}

impl Default for AggregateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            window_seconds: 30,
            group_by: "alertname".into(),
            mode: AggregateMode::HeadSummary,
            sample_labels: vec!["ip".into(), "instance".into(), "alertIp".into()],
            sample_limit: 10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregatePushResult {
    /// Aggregation off — caller should notify normally.
    Bypass,
    /// First event in window (head+summary) — send this alert now.
    SendHead,
    /// Counted into the bucket — do not send individually.
    Buffered,
}

#[derive(Debug, Clone)]
pub struct AggregateSummary {
    pub channel_id: String,
    pub group_key: String,
    pub count: u32,
    pub samples: Vec<String>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct AggBucket {
    window_start: DateTime<Utc>,
    count: u32,
    samples: Vec<String>,
    fingerprints: Vec<String>,
    head_sent: bool,
}

/// In-process aggregation buckets keyed by `channel_id|group_key`.
#[derive(Debug)]
pub struct AggregateBuffer {
    config: AggregateConfig,
    buckets: Mutex<HashMap<String, AggBucket>>,
}

impl AggregateBuffer {
    pub fn new(config: AggregateConfig) -> Self {
        Self {
            config,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &AggregateConfig {
        &self.config
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// Record a BecameFiring candidate for aggregation.
    pub fn push(
        &self,
        channel_id: &str,
        group_key: &str,
        fingerprint: &str,
        sample: impl Into<String>,
        now: DateTime<Utc>,
    ) -> AggregatePushResult {
        if !self.config.enabled {
            return AggregatePushResult::Bypass;
        }
        let sample = sample.into();
        let map_key = format!("{channel_id}|{group_key}");
        let window = Duration::seconds(self.config.window_seconds.max(1) as i64);
        let limit = self.config.sample_limit.max(1);

        let mut buckets = self.buckets.lock().expect("aggregate mutex");
        if let Some(b) = buckets.get_mut(&map_key) {
            if now < b.window_start + window {
                b.count = b.count.saturating_add(1);
                if b.samples.len() < limit && !sample.is_empty() {
                    b.samples.push(sample);
                }
                if b.fingerprints.len() < limit {
                    b.fingerprints.push(fingerprint.to_string());
                }
                return AggregatePushResult::Buffered;
            }
            // Previous window should have been flushed; start fresh.
        }

        let head = matches!(self.config.mode, AggregateMode::HeadSummary);
        let mut samples = Vec::new();
        if !sample.is_empty() {
            samples.push(sample);
        }
        buckets.insert(
            map_key,
            AggBucket {
                window_start: now,
                count: 1,
                samples,
                fingerprints: vec![fingerprint.to_string()],
                head_sent: head,
            },
        );
        if head {
            AggregatePushResult::SendHead
        } else {
            AggregatePushResult::Buffered
        }
    }

    /// Pop buckets whose window has ended.
    pub fn take_due(&self, now: DateTime<Utc>) -> Vec<AggregateSummary> {
        if !self.config.enabled {
            return Vec::new();
        }
        let window = Duration::seconds(self.config.window_seconds.max(1) as i64);
        let mut buckets = self.buckets.lock().expect("aggregate mutex");
        let mut due_keys = Vec::new();
        for (k, b) in buckets.iter() {
            if now >= b.window_start + window {
                due_keys.push(k.clone());
            }
        }
        let mut out = Vec::new();
        for k in due_keys {
            let Some(b) = buckets.remove(&k) else {
                continue;
            };
            // head+summary with only the head: skip redundant summary.
            if matches!(self.config.mode, AggregateMode::HeadSummary)
                && b.head_sent
                && b.count <= 1
            {
                continue;
            }
            if b.count == 0 {
                continue;
            }
            let (channel_id, group_key) = k.split_once('|').unwrap_or((k.as_str(), ""));
            out.push(AggregateSummary {
                channel_id: channel_id.to_string(),
                group_key: group_key.to_string(),
                count: b.count,
                samples: b.samples,
                window_start: b.window_start,
                window_end: now,
            });
        }
        out
    }
}

/// Join `group_by` label values (comma-separated names).
pub fn build_group_key(group_by: &str, labels: &Labels) -> String {
    let parts: Vec<String> = group_by
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|k| labels.get(k).cloned().unwrap_or_default())
        .collect();
    let joined = parts.join("|");
    if joined.is_empty() || joined.chars().all(|c| c == '|') {
        labels
            .get("alertname")
            .cloned()
            .unwrap_or_else(|| "unknown".into())
    } else {
        joined
    }
}

/// Pick first non-empty sample label for aggregate preview lists.
pub fn format_aggregate_sample(sample_labels: &[String], labels: &Labels) -> String {
    for k in sample_labels {
        if let Some(v) = labels.get(k) {
            if !v.is_empty() {
                return v.clone();
            }
        }
    }
    String::new()
}

pub fn format_aggregate_text(summary: &AggregateSummary) -> String {
    let samples = if summary.samples.is_empty() {
        "(无样例)".to_string()
    } else {
        summary.samples.join(", ")
    };
    format!(
        "[聚合告警] {}\ncount: {}\nwindow: {} .. {}\nsamples: {}\n",
        summary.group_key,
        summary.count,
        summary.window_start.to_rfc3339(),
        summary.window_end.to_rfc3339(),
        samples
    )
}

// --- P2: ingress inflight limit + notify degrade ---

#[derive(Debug, Clone)]
pub struct IngressPressureConfig {
    /// Max concurrent HTTP ingress handlers. `0` = unlimited.
    pub max_inflight: usize,
    /// When true, skip notifier under pressure (still persist alerts).
    pub degrade_skip_notify: bool,
    /// Notify attempts/sec above this triggers degrade (when degrade_skip_notify).
    pub degrade_notify_per_sec: u32,
}

impl Default for IngressPressureConfig {
    fn default() -> Self {
        Self {
            max_inflight: 100,
            degrade_skip_notify: false,
            degrade_notify_per_sec: 50,
        }
    }
}

/// Tracks ingress concurrency and recent notify attempt rate.
#[derive(Debug)]
pub struct IngressPressure {
    config: IngressPressureConfig,
    inflight: std::sync::atomic::AtomicUsize,
    rate: Mutex<(DateTime<Utc>, u32)>,
}

/// RAII permit: drops decrement inflight.
#[derive(Debug)]
pub struct IngressPermit<'a> {
    pressure: &'a IngressPressure,
}

impl Drop for IngressPermit<'_> {
    fn drop(&mut self) {
        self.pressure.leave();
    }
}

impl IngressPressure {
    pub fn new(config: IngressPressureConfig) -> Self {
        Self {
            config,
            inflight: std::sync::atomic::AtomicUsize::new(0),
            rate: Mutex::new((Utc::now(), 0)),
        }
    }

    pub fn config(&self) -> &IngressPressureConfig {
        &self.config
    }

    pub fn inflight(&self) -> usize {
        self.inflight.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Acquire a permit or `None` if at capacity (caller should return HTTP 429).
    pub fn try_enter(&self) -> Option<IngressPermit<'_>> {
        use std::sync::atomic::Ordering;
        if self.config.max_inflight == 0 {
            self.inflight.fetch_add(1, Ordering::SeqCst);
            return Some(IngressPermit { pressure: self });
        }
        loop {
            let cur = self.inflight.load(Ordering::Relaxed);
            if cur >= self.config.max_inflight {
                return None;
            }
            if self
                .inflight
                .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return Some(IngressPermit { pressure: self });
            }
        }
    }

    fn leave(&self) {
        self.inflight
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Count a would-be notification (call when should_notify is true).
    pub fn note_notify_attempt(&self, now: DateTime<Utc>) {
        let mut g = self.rate.lock().expect("pressure rate");
        if now >= g.0 + Duration::seconds(1) {
            g.0 = now;
            g.1 = 1;
        } else {
            g.1 = g.1.saturating_add(1);
        }
    }

    pub fn notify_rate(&self, now: DateTime<Utc>) -> u32 {
        let g = self.rate.lock().expect("pressure rate");
        if now >= g.0 + Duration::seconds(1) {
            0
        } else {
            g.1
        }
    }

    /// Whether to skip sending (persist still happens).
    pub fn should_skip_notify(&self, now: DateTime<Utc>) -> bool {
        if !self.config.degrade_skip_notify {
            return false;
        }
        let inflight = self.inflight();
        let busy = if self.config.max_inflight == 0 {
            false
        } else {
            inflight * 10 >= self.config.max_inflight * 8
        };
        let hot = self.notify_rate(now) >= self.config.degrade_notify_per_sec.max(1);
        busy || hot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn gate(max: u32, window: u64, min_interval: u64) -> ThrottleGate {
        ThrottleGate::new(ThrottleConfig {
            enabled: true,
            min_interval_seconds: min_interval,
            max_per_window: max,
            window_seconds: window,
            resolve_max_per_window: max,
        })
    }

    #[test]
    fn window_limit_denies_after_max() {
        let g = gate(20, 60, 0);
        let t0 = Utc::now();
        for i in 0..20 {
            let d = g.allow(
                "ch1",
                "fp1",
                AlertTransition::BecameFiring,
                t0 + Duration::milliseconds(i),
            );
            assert_eq!(d, ThrottleDecision::Allow, "i={i}");
        }
        let denied = g.allow(
            "ch1",
            "fp1",
            AlertTransition::BecameFiring,
            t0 + Duration::milliseconds(21),
        );
        assert_eq!(denied, ThrottleDecision::DenyWindowLimit);
    }

    #[test]
    fn min_interval_denies_bursts() {
        let g = gate(100, 3600, 60);
        let t0 = Utc::now();
        assert_eq!(
            g.allow("ch", "k", AlertTransition::BecameFiring, t0),
            ThrottleDecision::Allow
        );
        assert_eq!(
            g.allow(
                "ch",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(30)
            ),
            ThrottleDecision::DenyMinInterval
        );
        assert_eq!(
            g.allow(
                "ch",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(60)
            ),
            ThrottleDecision::Allow
        );
    }

    #[test]
    fn window_rolls_and_allows_again() {
        let g = gate(2, 10, 0);
        let t0 = Utc::now();
        assert_eq!(
            g.allow("c", "k", AlertTransition::BecameFiring, t0),
            ThrottleDecision::Allow
        );
        assert_eq!(
            g.allow(
                "c",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(1)
            ),
            ThrottleDecision::Allow
        );
        assert_eq!(
            g.allow(
                "c",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(2)
            ),
            ThrottleDecision::DenyWindowLimit
        );
        assert_eq!(
            g.allow(
                "c",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(11)
            ),
            ThrottleDecision::Allow
        );
    }

    #[test]
    fn firing_and_resolved_counters_are_separate() {
        let g = gate(1, 60, 0);
        let t0 = Utc::now();
        assert_eq!(
            g.allow("c", "k", AlertTransition::BecameFiring, t0),
            ThrottleDecision::Allow
        );
        assert_eq!(
            g.allow(
                "c",
                "k",
                AlertTransition::BecameFiring,
                t0 + Duration::seconds(1)
            ),
            ThrottleDecision::DenyWindowLimit
        );
        assert_eq!(
            g.allow(
                "c",
                "k",
                AlertTransition::BecameResolved,
                t0 + Duration::seconds(1)
            ),
            ThrottleDecision::Allow
        );
    }

    #[test]
    fn build_key_from_labels() {
        let mut labels = BTreeMap::new();
        labels.insert("alertname".into(), "DiskFull".into());
        labels.insert("ip".into(), "10.0.0.1".into());
        assert_eq!(
            build_throttle_key("labels:alertname,ip", "fp", &labels),
            "DiskFull|10.0.0.1"
        );
        assert_eq!(build_throttle_key("fingerprint", "fp", &labels), "fp");
    }

    fn agg_buf(mode: AggregateMode, window: u64) -> AggregateBuffer {
        AggregateBuffer::new(AggregateConfig {
            enabled: true,
            window_seconds: window,
            group_by: "alertname".into(),
            mode,
            sample_labels: vec!["ip".into()],
            sample_limit: 10,
        })
    }

    #[test]
    fn head_summary_buffers_after_first() {
        let buf = agg_buf(AggregateMode::HeadSummary, 30);
        let t0 = Utc::now();
        assert_eq!(
            buf.push("ch", "DiskFull", "a", "10.0.0.1", t0),
            AggregatePushResult::SendHead
        );
        assert_eq!(
            buf.push("ch", "DiskFull", "b", "10.0.0.2", t0 + Duration::seconds(1)),
            AggregatePushResult::Buffered
        );
        let due = buf.take_due(t0 + Duration::seconds(31));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].count, 2);
        assert_eq!(due[0].samples.len(), 2);
    }

    #[test]
    fn summary_only_never_sends_head() {
        let buf = agg_buf(AggregateMode::SummaryOnly, 10);
        let t0 = Utc::now();
        assert_eq!(
            buf.push("ch", "g", "a", "1", t0),
            AggregatePushResult::Buffered
        );
        assert_eq!(
            buf.push("ch", "g", "b", "2", t0 + Duration::seconds(1)),
            AggregatePushResult::Buffered
        );
        let due = buf.take_due(t0 + Duration::seconds(11));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].count, 2);
    }

    #[test]
    fn head_only_one_skips_summary() {
        let buf = agg_buf(AggregateMode::HeadSummary, 5);
        let t0 = Utc::now();
        assert_eq!(
            buf.push("ch", "g", "a", "1", t0),
            AggregatePushResult::SendHead
        );
        assert!(buf.take_due(t0 + Duration::seconds(6)).is_empty());
    }

    #[test]
    fn build_group_and_sample() {
        let mut labels = BTreeMap::new();
        labels.insert("alertname".into(), "Disk".into());
        labels.insert("ip".into(), "1.1.1.1".into());
        assert_eq!(build_group_key("alertname", &labels), "Disk");
        assert_eq!(
            format_aggregate_sample(&["ip".into(), "instance".into()], &labels),
            "1.1.1.1"
        );
    }

    #[test]
    fn ingress_inflight_rejects_at_capacity() {
        let p = IngressPressure::new(IngressPressureConfig {
            max_inflight: 2,
            degrade_skip_notify: false,
            degrade_notify_per_sec: 50,
        });
        let a = p.try_enter().expect("a");
        let b = p.try_enter().expect("b");
        assert!(p.try_enter().is_none());
        drop(a);
        assert!(p.try_enter().is_some());
        drop(b);
    }

    #[test]
    fn degrade_skips_when_notify_rate_hot() {
        let p = IngressPressure::new(IngressPressureConfig {
            max_inflight: 100,
            degrade_skip_notify: true,
            degrade_notify_per_sec: 3,
        });
        let t0 = Utc::now();
        assert!(!p.should_skip_notify(t0));
        p.note_notify_attempt(t0);
        p.note_notify_attempt(t0);
        p.note_notify_attempt(t0);
        assert!(p.should_skip_notify(t0));
    }
}
