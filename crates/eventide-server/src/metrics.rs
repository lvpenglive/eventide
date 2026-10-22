//! Process-local runtime metrics (Prometheus text exposition).

use crate::state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Metrics {
    pub notify_success: AtomicU64,
    pub notify_failure: AtomicU64,
    pub notify_throttled: AtomicU64,
    pub notify_degraded: AtomicU64,
    pub ingress_accepted: AtomicU64,
    pub ingress_rejected_429: AtomicU64,
    pub escalation_sent: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc_notify_success(&self) {
        self.notify_success.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_notify_failure(&self) {
        self.notify_failure.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_notify_throttled(&self) {
        self.notify_throttled.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_notify_degraded(&self) {
        self.notify_degraded.fetch_add(1, Ordering::Relaxed);
    }
    pub fn add_ingress_accepted(&self, n: u64) {
        if n > 0 {
            self.ingress_accepted.fetch_add(n, Ordering::Relaxed);
        }
    }
    pub fn inc_ingress_rejected_429(&self) {
        self.ingress_rejected_429.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_escalation_sent(&self) {
        self.escalation_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot_json(&self, state: &AppState) -> serde_json::Value {
        serde_json::json!({
            "notify_success": self.notify_success.load(Ordering::Relaxed),
            "notify_failure": self.notify_failure.load(Ordering::Relaxed),
            "notify_throttled": self.notify_throttled.load(Ordering::Relaxed),
            "notify_degraded": self.notify_degraded.load(Ordering::Relaxed),
            "ingress_accepted": self.ingress_accepted.load(Ordering::Relaxed),
            "ingress_rejected_429": self.ingress_rejected_429.load(Ordering::Relaxed),
            "escalation_sent": self.escalation_sent.load(Ordering::Relaxed),
            "pressure_inflight": state.pressure.inflight(),
            "ingress_max_inflight": state
                .storm
                .read()
                .map(|s| s.ingress_max_inflight)
                .unwrap_or(state.config.storm.ingress_max_inflight),
            "is_leader": state.leader.is_leader(),
            "cluster_enabled": state.config.cluster.enabled,
        })
    }

    pub fn render_prometheus(&self, state: &AppState) -> String {
        let mut out = String::with_capacity(2048);
        fn counter(buf: &mut String, name: &str, help: &str, v: u64) {
            buf.push_str("# HELP ");
            buf.push_str(name);
            buf.push(' ');
            buf.push_str(help);
            buf.push('\n');
            buf.push_str("# TYPE ");
            buf.push_str(name);
            buf.push_str(" counter\n");
            buf.push_str(name);
            buf.push(' ');
            buf.push_str(&v.to_string());
            buf.push('\n');
        }
        fn gauge(buf: &mut String, name: &str, help: &str, v: u64) {
            buf.push_str("# HELP ");
            buf.push_str(name);
            buf.push(' ');
            buf.push_str(help);
            buf.push('\n');
            buf.push_str("# TYPE ");
            buf.push_str(name);
            buf.push_str(" gauge\n");
            buf.push_str(name);
            buf.push(' ');
            buf.push_str(&v.to_string());
            buf.push('\n');
        }

        counter(
            &mut out,
            "eventide_notify_success_total",
            "Successful notify deliveries since process start",
            self.notify_success.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_notify_failure_total",
            "Failed notify deliveries since process start",
            self.notify_failure.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_notify_throttled_total",
            "Notify skipped by storm throttle",
            self.notify_throttled.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_notify_degraded_total",
            "Notify skipped by storm degrade mode",
            self.notify_degraded.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_ingress_accepted_total",
            "HTTP ingress alerts accepted",
            self.ingress_accepted.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_ingress_rejected_429_total",
            "HTTP ingress rejected due to inflight pressure",
            self.ingress_rejected_429.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "eventide_escalation_sent_total",
            "Unacked timeout escalations sent",
            self.escalation_sent.load(Ordering::Relaxed),
        );
        gauge(
            &mut out,
            "eventide_ingress_inflight",
            "Current HTTP ingress in-flight requests",
            state.pressure.inflight() as u64,
        );
        gauge(
            &mut out,
            "eventide_ingress_max_inflight",
            "Configured HTTP ingress max in-flight",
            state
                .storm
                .read()
                .map(|s| s.ingress_max_inflight as u64)
                .unwrap_or(state.config.storm.ingress_max_inflight as u64),
        );
        gauge(
            &mut out,
            "eventide_is_leader",
            "1 if this process holds the cluster lease",
            if state.leader.is_leader() { 1 } else { 0 },
        );
        gauge(
            &mut out,
            "eventide_cluster_enabled",
            "1 if cluster mode is enabled",
            if state.config.cluster.enabled { 1 } else { 0 },
        );
        out
    }
}
