//! Optional Elasticsearch client for alert history (index + search).

use eventide_core::AlertEvent;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EsConfig {
    pub url: String,
    pub index: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone)]
pub struct EsClient {
    http: Client,
    cfg: EsConfig,
}

impl EsClient {
    pub fn new(cfg: EsConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self { http, cfg })
    }

    fn url(&self, path: &str) -> String {
        let base = self.cfg.url.trim_end_matches('/');
        format!("{base}{path}")
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match (&self.cfg.username, &self.cfg.password) {
            (Some(u), Some(p)) if !u.is_empty() => req.basic_auth(u, Some(p)),
            (Some(u), None) if !u.is_empty() => req.basic_auth(u, None::<&str>),
            _ => req,
        }
    }

    /// Upsert one alert document (id = alert uuid).
    pub async fn index_alert(&self, event: &AlertEvent) -> anyhow::Result<()> {
        let mut doc = serde_json::to_value(event)?;
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("search_text".into(), json!(search_blob(event)));
        }
        let url = self.url(&format!(
            "/{}/_doc/{}",
            self.cfg.index,
            event.id
        ));
        let req = self.apply_auth(self.http.put(url).json(&doc));
        let res = req.send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("elasticsearch index {status}: {body}");
        }
        Ok(())
    }

    /// Search alerts; returns deserialized AlertEvent list (best-effort).
    /// IP filter is applied in-process after fetch (label fields vary).
    pub async fn search_alerts(
        &self,
        status: Option<&str>,
        severity: Option<&str>,
        source: Option<&str>,
        q: Option<&str>,
        ip: Option<&str>,
        size: usize,
    ) -> anyhow::Result<Vec<AlertEvent>> {
        let mut filter = Vec::new();
        if let Some(s) = status.filter(|s| !s.is_empty()) {
            filter.push(json!({ "term": { "status": s } }));
        }
        if let Some(s) = severity.filter(|s| !s.is_empty()) {
            filter.push(json!({ "term": { "severity": s } }));
        }
        if let Some(src) = source.filter(|s| !s.is_empty()) {
            match src {
                "ingress" => filter.push(json!({
                    "prefix": { "labels.source": "ingress:" }
                })),
                "rule" => filter.push(json!({
                    "bool": {
                        "must_not": [{ "prefix": { "labels.source": "ingress:" } }]
                    }
                })),
                _ => {}
            }
        }

        let mut must = Vec::new();
        if let Some(needle) = q.map(str::trim).filter(|s| !s.is_empty()) {
            must.push(json!({
                "simple_query_string": {
                    "query": format!("*{needle}*"),
                    "fields": ["search_text", "fingerprint", "labels.*", "annotations.*"],
                    "default_operator": "and"
                }
            }));
        }

        let query = if filter.is_empty() && must.is_empty() {
            json!({ "match_all": {} })
        } else {
            json!({
                "bool": {
                    "filter": filter,
                    "must": must
                }
            })
        };

        let body = json!({
            "size": size.max(1).min(1000),
            "sort": [
                { "last_evaluated_at": { "order": "desc", "unmapped_type": "date" } }
            ],
            "query": query
        });

        let url = self.url(&format!("/{}/_search", self.cfg.index));
        let req = self.apply_auth(self.http.post(url).json(&body));
        let res = req.send().await?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("elasticsearch search {status}: {text}");
        }
        let v: Value = serde_json::from_str(&text)?;
        let hits = v
            .pointer("/hits/hits")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::with_capacity(hits.len());
        for hit in hits {
            let Some(src) = hit.get("_source") else {
                continue;
            };
            // Drop helper field before deserialize if present.
            let mut src = src.clone();
            if let Some(obj) = src.as_object_mut() {
                obj.remove("search_text");
            }
            match serde_json::from_value::<AlertEvent>(src) {
                Ok(ev) => out.push(ev),
                Err(e) => tracing::warn!("skip bad es alert doc: {e}"),
            }
        }
        if let Some(ip) = ip.map(str::trim).filter(|s| !s.is_empty()) {
            let n = ip.to_ascii_lowercase();
            const KEYS: &[&str] = &[
                "alertIp", "ip", "ipaddr", "instance", "host", "hostname",
            ];
            out.retain(|a| {
                KEYS.iter().any(|k| {
                    a.labels
                        .get(*k)
                        .map(|v| v.to_ascii_lowercase().contains(&n))
                        .unwrap_or(false)
                })
            });
        }
        Ok(out)
    }
}

fn search_blob(event: &AlertEvent) -> String {
    let mut parts = Vec::new();
    parts.push(event.fingerprint.clone());
    parts.push(event.status.as_str().to_string());
    parts.push(event.severity.as_str().to_string());
    for (k, v) in &event.labels {
        parts.push(format!("{k}={v}"));
        parts.push(v.clone());
    }
    for (k, v) in &event.annotations {
        parts.push(format!("{k}={v}"));
        parts.push(v.clone());
    }
    parts.join(" ")
}
