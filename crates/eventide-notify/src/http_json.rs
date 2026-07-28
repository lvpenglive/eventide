//! Custom HTTP JSON notifier (user-defined body template + headers).

use crate::channel_opts::option;
use crate::NotifyError;
use eventide_core::{
    format_notify_json_for_channel, AlertEvent, AlertTransition, NotifyChannel, Rule,
};

pub struct HttpNotifier;

impl HttpNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let body = format_notify_json_for_channel(channel, rule, event, transition)
            .map_err(NotifyError::Channel)?;
        let report = Self::send_json_report(http, channel, body).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report.error.unwrap_or_else(|| "http channel failed".into()),
            ))
        }
    }

    pub async fn send_text(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> Result<(), NotifyError> {
        let report = Self::send_text_report(http, channel, content).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report.error.unwrap_or_else(|| "http channel failed".into()),
            ))
        }
    }

    pub async fn send_text_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> crate::NotifySendReport {
        // Prefer rendering json template with text-only fallback context is hard;
        // use a simple envelope for aggregate / channel test.
        let body = serde_json::json!({
            "transition": "test",
            "text": content,
        });
        Self::send_json_report(http, channel, body).await
    }

    pub async fn send_json_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        body: serde_json::Value,
    ) -> crate::NotifySendReport {
        let method = option(channel, "http_method")
            .unwrap_or("POST")
            .to_ascii_uppercase();
        let mut req = match method.as_str() {
            "PUT" => http.put(&channel.url),
            "PATCH" => http.patch(&channel.url),
            _ => http.post(&channel.url),
        };
        req = req.json(&body);

        if let Some(secret) = channel.secret.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            req = req.bearer_auth(secret);
        }

        for (name, value) in parse_headers(channel) {
            req = req.header(name, value);
        }

        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                let http_status = status.as_u16();
                let response_body = resp.text().await.unwrap_or_default();
                let ok = status.is_success();
                crate::NotifySendReport {
                    ok,
                    kind: "http".into(),
                    request_url: channel.url.clone(),
                    request_body: body,
                    http_status: Some(http_status),
                    response_body,
                    error: if ok {
                        None
                    } else {
                        Some(format!("http {http_status}"))
                    },
                }
            }
            Err(e) => crate::NotifySendReport {
                ok: false,
                kind: "http".into(),
                request_url: channel.url.clone(),
                request_body: body,
                http_status: None,
                response_body: String::new(),
                error: Some(e.to_string()),
            },
        }
    }
}

fn parse_headers(channel: &NotifyChannel) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(raw) = option(channel, "headers_json") {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(raw) {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    out.push((k, s.to_string()));
                } else if !v.is_null() {
                    out.push((k, v.to_string()));
                }
            }
        }
    }
    // Also allow header_Name = value options.
    for (k, v) in &channel.options {
        if let Some(name) = k.strip_prefix("header_") {
            let name = name.trim();
            if !name.is_empty() && !v.trim().is_empty() {
                out.push((name.to_string(), v.trim().to_string()));
            }
        }
    }
    out
}
