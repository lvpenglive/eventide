//! Datasource adapters: Prometheus, Loki/Log, Kafka.

mod kafka;
mod log;
mod prometheus;

pub use kafka::{
    browse_messages, connect_client, create_topic, delete_topic, describe_topic, earliest_offset,
    fetch_records_from, latest_offset, list_topics, produce_message, topic_partition_count,
    BrowsedMessage, KafkaClientPool, KafkaError, KafkaSource, PartitionOffsets, TopicInfo,
};
pub use log::{LogClient, LogError};
pub use prometheus::{PrometheusClient, PrometheusError, QueryResult};

use eventide_core::{Datasource, DatasourceKind, MetricSample};

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error(transparent)]
    Prometheus(#[from] PrometheusError),
    #[error(transparent)]
    Log(#[from] LogError),
    #[error(transparent)]
    Kafka(#[from] KafkaError),
    #[error("unsupported datasource kind: {0}")]
    Unsupported(String),
}

/// Fetch metric samples for a rule expression against a datasource.
pub async fn fetch_samples(
    ds: &Datasource,
    expr: &str,
) -> Result<Vec<MetricSample>, SourceError> {
    match ds.kind {
        DatasourceKind::Prometheus | DatasourceKind::VictoriaMetrics => {
            let client = PrometheusClient::new(&ds.url)?;
            Ok(client.instant_query(expr).await?)
        }
        DatasourceKind::Log => {
            let client = LogClient::new(&ds.url)?;
            Ok(client.query(expr).await?)
        }
        DatasourceKind::Kafka => {
            let src = KafkaSource::new(&ds.url);
            let topic = ds
                .options
                .get("topic")
                .map(|s| s.as_str())
                .unwrap_or("")
                .trim();
            let mode = ds
                .options
                .get("mode")
                .map(|s| s.as_str())
                .unwrap_or("field");

            match mode {
                "depth" => Ok(src.topic_depth(topic).await?),
                "count" | "recent" | "messages" => {
                    let max = ds
                        .options
                        .get("max_records")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(100);
                    Ok(src.recent_count(topic, max).await?)
                }
                // Default: parse JSON field from messages (data pipeline).
                _ => {
                    // Rule expr overrides field path; options.field is fallback.
                    let field = if !expr.trim().is_empty() {
                        expr.trim()
                    } else {
                        ds.options
                            .get("field")
                            .map(|s| s.as_str())
                            .unwrap_or("")
                            .trim()
                    };
                    let label_fields: Vec<String> = ds
                        .options
                        .get("label_fields")
                        .map(|s| {
                            s.split(',')
                                .map(|x| x.trim().to_string())
                                .filter(|x| !x.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    let max_records = ds
                        .options
                        .get("max_records")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(100);
                    let max_partitions = ds
                        .options
                        .get("partitions")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(8);
                    Ok(src
                        .field_samples(
                            topic,
                            field,
                            &label_fields,
                            max_records,
                            max_partitions,
                        )
                        .await?)
                }
            }
        }
    }
}
