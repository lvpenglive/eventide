//! In-memory counters + recent alerts ring.

use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

#[derive(Default)]
pub struct TrapStats {
    pub received: AtomicU64,
    pub parsed_ok: AtomicU64,
    pub parse_err: AtomicU64,
    pub kafka_ok: AtomicU64,
    pub kafka_err: AtomicU64,
    pub simulated: AtomicU64,
}

#[derive(Clone, Serialize)]
pub struct RecentItem {
    pub at: String,
    pub peer: String,
    pub trap_oid: String,
    pub alertname: String,
    pub kafka: bool,
    pub alert: Value,
}

pub struct RecentBuffer {
    limit: usize,
    items: RwLock<VecDeque<RecentItem>>,
}

impl RecentBuffer {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            items: RwLock::new(VecDeque::new()),
        }
    }

    pub async fn push(&self, item: RecentItem) {
        let mut q = self.items.write().await;
        q.push_front(item);
        while q.len() > self.limit {
            q.pop_back();
        }
    }

    pub async fn list(&self) -> Vec<RecentItem> {
        self.items.read().await.iter().cloned().collect()
    }
}

impl TrapStats {
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "received": self.received.load(Ordering::Relaxed),
            "parsed_ok": self.parsed_ok.load(Ordering::Relaxed),
            "parse_err": self.parse_err.load(Ordering::Relaxed),
            "kafka_ok": self.kafka_ok.load(Ordering::Relaxed),
            "kafka_err": self.kafka_err.load(Ordering::Relaxed),
            "simulated": self.simulated.load(Ordering::Relaxed),
        })
    }
}
