//! Kafka producer for normalized Trap alerts.

use anyhow::{Context, Result};
use chrono::Utc;
use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use rskafka::record::Record;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::Mutex;

struct KafkaInner {
    client: Client,
    partitions: HashMap<i32, Arc<rskafka::client::partition::PartitionClient>>,
}

pub struct KafkaOut {
    brokers: Vec<String>,
    topic: String,
    /// When > 1, produce to `hash(key) % partitions`.
    partitions: i32,
    /// Used only when `partitions == 1` (compat with fixed `kafka_partition`).
    fixed_partition: i32,
    inner: Mutex<Option<KafkaInner>>,
}

impl KafkaOut {
    pub fn new(brokers_csv: &str, topic: String, partitions: i32, fixed_partition: i32) -> Option<Self> {
        let brokers: Vec<String> = brokers_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if brokers.is_empty() || topic.is_empty() {
            return None;
        }
        let partitions = partitions.max(1);
        Some(Self {
            brokers,
            topic,
            partitions,
            fixed_partition,
            inner: Mutex::new(None),
        })
    }

    fn target_partition(&self, key: &str) -> i32 {
        if self.partitions <= 1 {
            return self.fixed_partition.max(0);
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % self.partitions as u64) as i32
    }

    async fn partition_client(
        &self,
        partition: i32,
    ) -> Result<Arc<rskafka::client::partition::PartitionClient>> {
        let mut guard = self.inner.lock().await;
        if let Some(inner) = guard.as_mut() {
            if let Some(pc) = inner.partitions.get(&partition) {
                return Ok(pc.clone());
            }
            let pc = inner
                .client
                .partition_client(
                    self.topic.clone(),
                    partition,
                    UnknownTopicHandling::Error,
                )
                .await
                .map_err(|e| anyhow::anyhow!("kafka partition {partition}: {e}"))?;
            let arc = Arc::new(pc);
            inner.partitions.insert(partition, arc.clone());
            return Ok(arc);
        }

        let client = ClientBuilder::new(self.brokers.clone())
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("kafka connect: {e}"))?;
        let pc = client
            .partition_client(
                self.topic.clone(),
                partition,
                UnknownTopicHandling::Error,
            )
            .await
            .map_err(|e| anyhow::anyhow!("kafka partition {partition}: {e}"))?;
        let arc = Arc::new(pc);
        let mut map = HashMap::new();
        map.insert(partition, arc.clone());
        *guard = Some(KafkaInner {
            client,
            partitions: map,
        });
        Ok(arc)
    }

    pub async fn publish(&self, key: &str, payload: Vec<u8>) -> Result<()> {
        let partition = self.target_partition(key);
        let pc = self
            .partition_client(partition)
            .await
            .context("kafka client")?;
        let record = Record {
            key: Some(key.as_bytes().to_vec()),
            value: Some(payload),
            headers: BTreeMap::new(),
            timestamp: Utc::now(),
        };
        pc.produce(vec![record], Compression::NoCompression)
            .await
            .map_err(|e| anyhow::anyhow!("kafka produce p{partition}: {e}"))?;
        Ok(())
    }
}
