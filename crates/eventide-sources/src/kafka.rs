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
