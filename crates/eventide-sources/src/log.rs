//! Loki / LogQL datasource adapter.

use chrono::{TimeZone, Utc};
use eventide_core::{Labels, MetricSample};
use serde::Deserialize;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("loki api error: {0}")]
    Api(String),
    #[error("unexpected response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone)]
pub struct LogClient {
    base_url: String,
    http: reqwest::Client,
}

impl LogClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, LogError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { base_url, http })
    }

    /// Run a Loki instant query (`/loki/api/v1/query`).
    ///
    /// Prefer LogQL that returns a vector/matrix of numbers, e.g.
    /// `count_over_time({app="api"} |= "error" [5m])`.
    pub async fn query(&self, expr: &str) -> Result<Vec<MetricSample>, LogError> {
        let url = format!("{}/loki/api/v1/query", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("query", expr)])
            .send()
            .await?
            .error_for_status()?;

        let body: LokiResponse = resp.json().await?;
        if body.status != "success" {
            return Err(LogError::Api(
                body.error.unwrap_or_else(|| body.status.clone()),
            ));
        }
        let data = body
            .data
            .ok_or_else(|| LogError::Unexpected("missing data".into()))?;

        let mut samples = Vec::new();
        for r in data.result {
            let (value, ts) = parse_loki_value(&r.value)?;
            let mut labels: Labels = BTreeMap::new();
            for (k, v) in r.metric.into_iter().chain(r.stream.into_iter()) {
                if k != "__name__" {
                    labels.insert(k, v);
                }
            }
            samples.push(MetricSample {
                labels,
                value,
                timestamp: ts,
            });
        }
        Ok(samples)
    }
}

#[derive(Debug, Deserialize)]
struct LokiResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    data: Option<LokiData>,
}

#[derive(Debug, Deserialize)]
struct LokiData {
    #[serde(default)]
    result: Vec<LokiResult>,
}

#[derive(Debug, Deserialize)]
struct LokiResult {
    #[serde(default)]
    metric: BTreeMap<String, String>,
    #[serde(default)]
    stream: BTreeMap<String, String>,
    #[serde(default)]
    value: serde_json::Value,
}

fn parse_loki_value(
    value: &serde_json::Value,
) -> Result<(f64, chrono::DateTime<Utc>), LogError> {
    // Instant vector: [timestamp, "value"] — same shape as Prometheus.
    let arr = value
        .as_array()
        .ok_or_else(|| LogError::Unexpected("value not array".into()))?;
    if arr.len() < 2 {
        return Err(LogError::Unexpected("value too short".into()));
    }
    let ts = arr[0]
        .as_f64()
        .ok_or_else(|| LogError::Unexpected("timestamp not number".into()))?;
    let s = arr[1]
        .as_str()
        .ok_or_else(|| LogError::Unexpected("value not string".into()))?;
    let v = s
        .parse::<f64>()
        .map_err(|e| LogError::Unexpected(format!("bad float: {e}")))?;
    let secs = ts.trunc() as i64;
    let nsecs = ((ts.fract()) * 1_000_000_000.0) as u32;
    let dt = Utc
        .timestamp_opt(secs, nsecs)
        .single()
        .ok_or_else(|| LogError::Unexpected("invalid timestamp".into()))?;
    Ok((v, dt))
}
