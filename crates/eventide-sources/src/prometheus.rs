//! Prometheus HTTP API client (instant query).

use chrono::{TimeZone, Utc};
use eventide_core::{Labels, MetricSample};
use serde::Deserialize;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrometheusError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("prometheus api error: {0}")]
    Api(String),
    #[error("unexpected response: {0}")]
    Unexpected(String),
}

#[derive(Debug, Clone)]
pub struct PrometheusClient {
    base_url: String,
    http: reqwest::Client,
}

impl PrometheusClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, PrometheusError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { base_url, http })
    }

    /// Run an instant query against `/api/v1/query`.
    pub async fn instant_query(&self, expr: &str) -> Result<Vec<MetricSample>, PrometheusError> {
        let url = format!("{}/api/v1/query", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("query", expr)])
            .send()
            .await?
            .error_for_status()?;

        let body: PromResponse = resp.json().await?;
        if body.status != "success" {
            return Err(PrometheusError::Api(
                body.error.unwrap_or_else(|| body.status.clone()),
            ));
        }

        let data = body
            .data
            .ok_or_else(|| PrometheusError::Unexpected("missing data".into()))?;

        let mut samples = Vec::new();
        for r in data.result {
            let value = parse_prom_value(&r.value)?;
            let ts = parse_prom_ts(&r.value)?;
            let mut labels: Labels = BTreeMap::new();
            for (k, v) in r.metric {
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
struct PromResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    data: Option<PromData>,
}

#[derive(Debug, Deserialize)]
struct PromData {
    #[serde(default)]
    result: Vec<PromResult>,
}

#[derive(Debug, Deserialize)]
struct PromResult {
    #[serde(default)]
    metric: BTreeMap<String, String>,
    value: serde_json::Value,
}

fn parse_prom_value(value: &serde_json::Value) -> Result<f64, PrometheusError> {
    // Instant vector: [timestamp, "value"]
    let arr = value
        .as_array()
        .ok_or_else(|| PrometheusError::Unexpected("value not array".into()))?;
    if arr.len() < 2 {
        return Err(PrometheusError::Unexpected("value too short".into()));
    }
    let s = arr[1]
        .as_str()
        .ok_or_else(|| PrometheusError::Unexpected("value not string".into()))?;
    s.parse::<f64>()
        .map_err(|e| PrometheusError::Unexpected(format!("bad float: {e}")))
}

fn parse_prom_ts(
    value: &serde_json::Value,
) -> Result<chrono::DateTime<Utc>, PrometheusError> {
    let arr = value
        .as_array()
        .ok_or_else(|| PrometheusError::Unexpected("value not array".into()))?;
    let ts = arr[0]
        .as_f64()
        .ok_or_else(|| PrometheusError::Unexpected("timestamp not number".into()))?;
    let secs = ts.trunc() as i64;
    let nsecs = ((ts.fract()) * 1_000_000_000.0) as u32;
    Utc.timestamp_opt(secs, nsecs)
        .single()
        .ok_or_else(|| PrometheusError::Unexpected("invalid timestamp".into()))
}

/// Parsed query result alias.
pub type QueryResult = Vec<MetricSample>;
