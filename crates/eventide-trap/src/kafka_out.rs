//! Kafka producer for normalized Trap alerts.

use anyhow::{Context, Result};
use chrono::Utc;
use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use rskafka::record::Record;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct KafkaOut {
    brokers: Vec<String>,
    topic: String,
    partition: i32,
    /// Lazily created partition client.
    inner: Mutex<Option<Arc<rskafka::client::partition::PartitionClient>>>,
}

impl KafkaOut {
    pub fn new(brokers_csv: &str, topic: String, partition: i32) -> Option<Self> {
        let brokers: Vec<String> = brokers_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if brokers.is_empty() || topic.is_empty() {
            return None;
        }
        Some(Self {
            brokers,
            topic,
            partition,
            inner: Mutex::new(None),
        })
    }

    async fn client(&self) -> Result<Arc<rskafka::client::partition::PartitionClient>> {
        let mut guard = self.inner.lock().await;
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
        let client = ClientBuilder::new(self.brokers.clone())
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("kafka connect: {e}"))?;
        let pc = client
            .partition_client(
                self.topic.clone(),
                self.partition,
                UnknownTopicHandling::Error,
            )
            .await
            .map_err(|e| anyhow::anyhow!("kafka partition {}: {e}", self.partition))?;
        let arc = Arc::new(pc);
        *guard = Some(arc.clone());
        Ok(arc)
    }

    pub async fn publish(&self, key: &str, payload: Vec<u8>) -> Result<()> {
        let pc = self.client().await.context("kafka client")?;
        let record = Record {
            key: Some(key.as_bytes().to_vec()),
            value: Some(payload),
            headers: BTreeMap::new(),
            timestamp: Utc::now(),
        };
        pc.produce(vec![record], Compression::NoCompression)
            .await
            .map_err(|e| anyhow::anyhow!("kafka produce: {e}"))?;
        Ok(())
    }
}
