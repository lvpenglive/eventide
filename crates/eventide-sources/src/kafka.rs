//! Kafka datasource: parse message JSON fields as metric samples (data pipeline).

use chrono::Utc;
use eventide_core::{Labels, MetricSample};
use rskafka::client::partition::{OffsetAt, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum KafkaError {
    #[error("kafka error: {0}")]
    Client(String),
    #[error("missing topic (set options.topic)")]
    MissingTopic,
    #[error("missing value field (set options.field or rule expr)")]
    MissingField,
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct KafkaSource {
    brokers: Vec<String>,
}

impl KafkaSource {
    pub fn new(brokers_csv: &str) -> Self {
        let brokers = brokers_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { brokers }
    }

    /// Consume recent messages and extract a numeric JSON field as samples.
    ///
    /// - `field`: dotted path, e.g. `latency_ms` or `metrics.p99`
    /// - `label_fields`: message fields copied into sample labels (for fingerprinting)
    pub async fn field_samples(
        &self,
        topic: &str,
        field: &str,
        label_fields: &[String],
        max_records: i32,
        max_partitions: i32,
    ) -> Result<Vec<MetricSample>, KafkaError> {
        if topic.is_empty() {
            return Err(KafkaError::MissingTopic);
        }
        if field.is_empty() {
            return Err(KafkaError::MissingField);
        }

        let client = ClientBuilder::new(self.brokers.clone())
            .build()
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;

        let now = Utc::now();
        // Deduplicate by labels within one evaluation — keep last value.
        let mut by_fp: BTreeMap<String, MetricSample> = BTreeMap::new();

        for partition in 0..max_partitions {
            let pc = match client
                .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
                .await
            {
                Ok(pc) => pc,
                Err(_) if partition == 0 => {
                    return Err(KafkaError::Client(format!(
                        "topic `{topic}` partition 0 unavailable"
                    )));
                }
                Err(_) => break,
            };

            let latest = pc
                .get_offset(OffsetAt::Latest)
                .await
                .map_err(|e| KafkaError::Client(e.to_string()))?;
            let earliest = pc
                .get_offset(OffsetAt::Earliest)
                .await
                .map_err(|e| KafkaError::Client(e.to_string()))?;
            if latest <= earliest {
                continue;
            }
            let start = (latest - max_records as i64).max(earliest);
            let (records, _) = pc
                .fetch_records(start, 1..2_000_000, 1_200)
                .await
                .map_err(|e| KafkaError::Client(e.to_string()))?;

            for rec in records {
                let raw = match rec.record.value {
                    Some(v) if !v.is_empty() => v,
                    _ => continue,
                };
                let text = match std::str::from_utf8(&raw) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let json: Value = match serde_json::from_str(text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let Some(value) = json_path_number(&json, field) else {
                    continue;
                };
                let mut labels: Labels = BTreeMap::new();
                labels.insert("topic".into(), topic.into());
                labels.insert("partition".into(), partition.to_string());
                for lf in label_fields {
                    if let Some(s) = json_path_string(&json, lf) {
                        labels.insert(lf.clone(), s);
                    }
                }
                let key = labels
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                by_fp.insert(
                    key,
                    MetricSample {
                        labels,
                        value,
                        timestamp: now,
                    },
                );
            }
        }

        Ok(by_fp.into_values().collect())
    }

    /// Topic backlog depth (ops metric — optional mode).
    pub async fn topic_depth(&self, topic: &str) -> Result<Vec<MetricSample>, KafkaError> {
        if topic.is_empty() {
            return Err(KafkaError::MissingTopic);
        }
        let client = ClientBuilder::new(self.brokers.clone())
            .build()
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;

        let mut samples = Vec::new();
        let mut total = 0.0_f64;
        let now = Utc::now();

        for partition in 0..64 {
            let pc = match client
                .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
                .await
            {
                Ok(pc) => pc,
                Err(_) if partition == 0 => {
                    return Err(KafkaError::Client(format!(
                        "topic `{topic}` partition 0 unavailable"
                    )));
                }
                Err(_) => break,
            };

            let earliest = pc
                .get_offset(OffsetAt::Earliest)
                .await
                .map_err(|e| KafkaError::Client(e.to_string()))?;
            let latest = pc
                .get_offset(OffsetAt::Latest)
                .await
                .map_err(|e| KafkaError::Client(e.to_string()))?;
            let depth = (latest - earliest).max(0) as f64;
            total += depth;

            let mut labels: Labels = BTreeMap::new();
            labels.insert("topic".into(), topic.into());
            labels.insert("partition".into(), partition.to_string());
            samples.push(MetricSample {
                labels,
                value: depth,
                timestamp: now,
            });
        }

        let mut labels: Labels = BTreeMap::new();
        labels.insert("topic".into(), topic.into());
        labels.insert("partition".into(), "all".into());
        samples.push(MetricSample {
            labels,
            value: total,
            timestamp: now,
        });
        Ok(samples)
    }

    /// Count of recent messages in the fetch window (ops / burst style).
    pub async fn recent_count(
        &self,
        topic: &str,
        max_records: i32,
    ) -> Result<Vec<MetricSample>, KafkaError> {
        if topic.is_empty() {
            return Err(KafkaError::MissingTopic);
        }
        let client = ClientBuilder::new(self.brokers.clone())
            .build()
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;
        let pc = client
            .partition_client(topic.to_string(), 0, UnknownTopicHandling::Error)
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;

        let latest = pc
            .get_offset(OffsetAt::Latest)
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;
        let earliest = pc
            .get_offset(OffsetAt::Earliest)
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;
        let start = (latest - max_records as i64).max(earliest);

        let (records, _) = pc
            .fetch_records(start, 1..1_000_000, 1_000)
            .await
            .map_err(|e| KafkaError::Client(e.to_string()))?;

        let mut labels: Labels = BTreeMap::new();
        labels.insert("topic".into(), topic.into());
        labels.insert("partition".into(), "0".into());
        Ok(vec![MetricSample {
            labels,
            value: records.len() as f64,
            timestamp: Utc::now(),
        }])
    }
}

fn json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn json_path_number(root: &Value, path: &str) -> Option<f64> {
    let v = json_path(root, path)?;
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn json_path_string(root: &Value, path: &str) -> Option<String> {
    let v = json_path(root, path)?;
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Shared Kafka clients keyed by bootstrap broker list (ingress / multi-tick reuse).
#[derive(Default)]
pub struct KafkaClientPool {
    clients: Mutex<HashMap<String, Arc<Client>>>,
}

impl KafkaClientPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, brokers: &[String]) -> Result<Arc<Client>, KafkaError> {
        let key = brokers_key(brokers);
        {
            let guard = self.clients.lock().await;
            if let Some(c) = guard.get(&key) {
                return Ok(Arc::clone(c));
            }
        }
        let client = Arc::new(connect_client(brokers.to_vec()).await?);
        let mut guard = self.clients.lock().await;
        Ok(Arc::clone(guard.entry(key).or_insert_with(|| Arc::clone(&client))))
    }
}

fn brokers_key(brokers: &[String]) -> String {
    let mut parts: Vec<&str> = brokers.iter().map(|s| s.as_str()).collect();
    parts.sort_unstable();
    parts.join(",")
}

pub async fn connect_client(brokers: Vec<String>) -> Result<Client, KafkaError> {
    ClientBuilder::new(brokers)
        .build()
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))
}

/// Resolve topic partition count via cluster metadata (`list_topics`).
pub async fn topic_partition_count(client: &Client, topic: &str) -> Result<i32, KafkaError> {
    let topic = topic.trim();
    if topic.is_empty() {
        return Err(KafkaError::MissingTopic);
    }
    let topics = list_topics(client).await?;
    let found = topics
        .into_iter()
        .find(|t| t.name == topic)
        .ok_or_else(|| KafkaError::Client(format!("topic `{topic}` not found in cluster metadata")))?;
    if found.partitions <= 0 {
        return Err(KafkaError::Client(format!(
            "topic `{topic}` reports zero partitions"
        )));
    }
    Ok(found.partitions)
}

#[derive(Debug, Clone)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: i32,
}

/// List topics and partition counts from cluster metadata.
pub async fn list_topics(client: &Client) -> Result<Vec<TopicInfo>, KafkaError> {
    let topics = client
        .list_topics()
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    let mut out: Vec<TopicInfo> = topics
        .into_iter()
        .map(|t| TopicInfo {
            name: t.name,
            partitions: t.partitions.len() as i32,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Create a topic (idempotent check left to caller).
pub async fn create_topic(
    client: &Client,
    topic: &str,
    partitions: i32,
    replication_factor: i16,
) -> Result<(), KafkaError> {
    let topic = topic.trim();
    if topic.is_empty() {
        return Err(KafkaError::MissingTopic);
    }
    if partitions <= 0 {
        return Err(KafkaError::Client("partitions must be > 0".into()));
    }
    let ctrl = client
        .controller_client()
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    ctrl.create_topic(topic, partitions, replication_factor.max(1), 30_000)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))
}

/// Delete a topic.
pub async fn delete_topic(client: &Client, topic: &str) -> Result<(), KafkaError> {
    let topic = topic.trim();
    if topic.is_empty() {
        return Err(KafkaError::MissingTopic);
    }
    let ctrl = client
        .controller_client()
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    ctrl.delete_topic(topic, 30_000)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))
}

#[derive(Debug, Clone)]
pub struct PartitionOffsets {
    pub partition: i32,
    pub earliest: i64,
    pub latest: i64,
}

/// Per-partition earliest/latest offsets for a topic.
pub async fn describe_topic(
    client: &Client,
    topic: &str,
) -> Result<Vec<PartitionOffsets>, KafkaError> {
    let info = list_topics(client).await?;
    let found = info
        .into_iter()
        .find(|t| t.name == topic)
        .ok_or_else(|| KafkaError::Client(format!("topic `{topic}` not found")))?;
    let mut out = Vec::with_capacity(found.partitions as usize);
    for p in 0..found.partitions {
        let earliest = earliest_offset(client, topic, p).await?;
        let latest = latest_offset(client, topic, p).await?;
        out.push(PartitionOffsets {
            partition: p,
            earliest,
            latest,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct BrowsedMessage {
    pub partition: i32,
    pub offset: i64,
    pub timestamp: String,
    pub key: Option<String>,
    pub value: String,
    pub value_bytes: usize,
}

fn bytes_preview(raw: Option<&[u8]>, max: usize) -> Option<String> {
    let b = raw?;
    if b.is_empty() {
        return Some(String::new());
    }
    match std::str::from_utf8(b) {
        Ok(s) => {
            if s.len() > max {
                Some(format!("{}…", &s[..max]))
            } else {
                Some(s.to_string())
            }
        }
        Err(_) => {
            let hex: String = b
                .iter()
                .take(64)
                .map(|x| format!("{x:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            Some(format!("<binary {hex}{}>", if b.len() > 64 { "…" } else { "" }))
        }
    }
}

/// Browse recent messages from one partition.
///
/// `from`: `latest` (last `max` msgs), `earliest`, or absolute `offset`.
pub async fn browse_messages(
    client: &Client,
    topic: &str,
    partition: i32,
    max: i32,
    from: &str,
    offset: Option<i64>,
) -> Result<(Vec<BrowsedMessage>, i64, i64), KafkaError> {
    let max = max.clamp(1, 200);
    let earliest = earliest_offset(client, topic, partition).await?;
    let latest = latest_offset(client, topic, partition).await?;
    let start = match from {
        "earliest" => earliest,
        "offset" => offset.unwrap_or(earliest).clamp(earliest, latest),
        _ => {
            // latest window
            (latest - max as i64).max(earliest)
        }
    };
    if start >= latest {
        return Ok((Vec::new(), earliest, latest));
    }

    let pc = client
        .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    let (records, _high) = pc
        .fetch_records(start, 1..4_000_000, 1_500)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;

    let mut out: Vec<BrowsedMessage> = records
        .into_iter()
        .take(max as usize)
        .map(|r| {
            let value_bytes = r.record.value.as_ref().map(|v| v.len()).unwrap_or(0);
            BrowsedMessage {
                partition,
                offset: r.offset,
                timestamp: r.record.timestamp.to_rfc3339(),
                key: bytes_preview(r.record.key.as_deref(), 256),
                value: bytes_preview(r.record.value.as_deref(), 8_192).unwrap_or_default(),
                value_bytes,
            }
        })
        .collect();
    // For "latest" window, show newest last (or reverse for UI newest-first)
    if from != "earliest" && from != "offset" {
        out.reverse();
    }
    Ok((out, earliest, latest))
}

/// Produce one message to a partition.
pub async fn produce_message(
    client: &Client,
    topic: &str,
    partition: i32,
    key: Option<&str>,
    value: &str,
) -> Result<(), KafkaError> {
    use rskafka::client::partition::Compression;
    use rskafka::record::Record;
    use std::collections::BTreeMap;

    let pc = client
        .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    let record = Record {
        key: key.map(|k| k.as_bytes().to_vec()),
        value: Some(value.as_bytes().to_vec()),
        headers: BTreeMap::new(),
        timestamp: Utc::now(),
    };
    pc.produce(vec![record], Compression::NoCompression)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    Ok(())
}

/// Fetch records for ingress (partition + starting offset).
///
/// `max_wait_ms` is kept low because the server already polls on an interval —
/// long blocking waits just inflate idle tick latency.
pub async fn fetch_records_from(
    client: &Client,
    topic: &str,
    partition: i32,
    offset: i64,
) -> Result<(Vec<Vec<u8>>, i64), KafkaError> {
    let pc = client
        .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    let (records, high) = pc
        .fetch_records(offset, 1..2_000_000, 200)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    let payloads = records
        .into_iter()
        .map(|r| r.record.value.unwrap_or_default())
        .collect();
    Ok((payloads, high))
}

pub async fn latest_offset(
    client: &Client,
    topic: &str,
    partition: i32,
) -> Result<i64, KafkaError> {
    let pc = client
        .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    pc.get_offset(OffsetAt::Latest)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))
}

pub async fn earliest_offset(
    client: &Client,
    topic: &str,
    partition: i32,
) -> Result<i64, KafkaError> {
    let pc = client
        .partition_client(topic.to_string(), partition, UnknownTopicHandling::Error)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))?;
    pc.get_offset(OffsetAt::Earliest)
        .await
        .map_err(|e| KafkaError::Client(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_nested_number() {
        let v = json!({"metrics": {"latency_ms": 820}});
        assert_eq!(json_path_number(&v, "metrics.latency_ms"), Some(820.0));
    }

    #[test]
    fn parse_bool_and_string() {
        let v = json!({"ok": true, "n": "3.5"});
        assert_eq!(json_path_number(&v, "ok"), Some(1.0));
        assert_eq!(json_path_number(&v, "n"), Some(3.5));
    }
}
