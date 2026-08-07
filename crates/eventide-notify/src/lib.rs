//! Notification channels: Webhook, DingTalk, WeCom, Feishu, Slack, Telegram, custom HTTP JSON.

mod channel_opts;
mod dingtalk;
mod feishu;
mod http_json;
mod slack;
mod telegram;
mod webhook;
mod wecom;

use eventide_core::{AlertEvent, AlertTransition, ChannelKind, NotifyChannel, Rule};
use serde::Serialize;
use thiserror::Error;

pub use dingtalk::DingTalkNotifier;
pub use feishu::FeishuNotifier;
pub use http_json::HttpNotifier;
pub use slack::SlackNotifier;
pub use telegram::TelegramNotifier;
pub use webhook::WebhookNotifier;
pub use wecom::WeComNotifier;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("channel error: {0}")]
    Channel(String),
    #[error("unsupported channel kind: {0}")]
    Unsupported(String),
}

/// Result of a probe / send attempt with request & response details.
#[derive(Debug, Clone, Serialize)]
pub struct NotifySendReport {
    pub ok: bool,
    pub kind: String,
    pub request_url: String,
    pub request_body: serde_json::Value,
    pub http_status: Option<u16>,
    pub response_body: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Notifier {
    http: reqwest::Client,
}

impl Notifier {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client");
        Self { http }
    }

    pub async fn send(
        &self,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        if !channel.enabled {
            return Ok(());
        }
        match channel.kind {
            ChannelKind::Webhook => {
                WebhookNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::DingTalk => {
                DingTalkNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::WeCom => {
                WeComNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::Feishu => {
                FeishuNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::Slack => {
                SlackNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::Telegram => {
                TelegramNotifier::send(&self.http, channel, rule, event, transition).await
            }
            ChannelKind::Http => {
                HttpNotifier::send(&self.http, channel, rule, event, transition).await
            }
        }
    }

    /// Send a preformatted text (e.g. storm aggregate summary).
    pub async fn send_text(
        &self,
        channel: &NotifyChannel,
        text: &str,
    ) -> Result<(), NotifyError> {
        let report = self.send_text_report(channel, text).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report
                    .error
                    .unwrap_or_else(|| "notify failed".into()),
            ))
        }
    }

    /// Send text and return request/response details (for channel test UI).
    /// Ignores `channel.enabled` so disabled channels can still be probed.
    pub async fn send_text_report(
        &self,
        channel: &NotifyChannel,
        text: &str,
    ) -> NotifySendReport {
        match channel.kind {
            ChannelKind::Webhook => {
                WebhookNotifier::send_text_report(&self.http, channel, text).await
            }
            ChannelKind::DingTalk => {
                DingTalkNotifier::send_text_report(&self.http, channel, text).await
            }
            ChannelKind::WeCom => WeComNotifier::send_text_report(&self.http, channel, text).await,
            ChannelKind::Feishu => {
                FeishuNotifier::send_text_report(&self.http, channel, text).await
            }
            ChannelKind::Slack => SlackNotifier::send_text_report(&self.http, channel, text).await,
            ChannelKind::Telegram => {
                TelegramNotifier::send_text_report(&self.http, channel, text).await
            }
            ChannelKind::Http => HttpNotifier::send_text_report(&self.http, channel, text).await,
        }
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn transition_label(t: AlertTransition) -> &'static str {
    match t {
        AlertTransition::BecameFiring => "firing",
        AlertTransition::BecameResolved => "resolved",
        AlertTransition::Escalated => "escalated",
        AlertTransition::Unchanged => "unchanged",
    }
}

pub(crate) fn build_text_for_channel(
    channel: &NotifyChannel,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> String {
    eventide_core::format_notify_text_for_channel(channel, rule, event, transition)
}

pub(crate) async fn post_json_report(
    http: &reqwest::Client,
    kind: &str,
    url: String,
    body: serde_json::Value,
    business_err: impl FnOnce(&str) -> Option<String>,
) -> NotifySendReport {
    match http.post(&url).json(&body).send().await {
        Ok(resp) => {
            let status = resp.status();
            let http_status = status.as_u16();
            let response_body = resp.text().await.unwrap_or_default();
            let mut ok = status.is_success();
            let mut error = if ok {
                None
            } else {
                Some(format!("http {http_status}"))
            };
            if ok {
                if let Some(biz) = business_err(&response_body) {
                    ok = false;
                    error = Some(biz);
                }
            }
            NotifySendReport {
                ok,
                kind: kind.to_string(),
                request_url: url,
                request_body: body,
                http_status: Some(http_status),
                response_body,
                error,
            }
        }
        Err(e) => NotifySendReport {
            ok: false,
            kind: kind.to_string(),
            request_url: url,
            request_body: body,
            http_status: None,
            response_body: String::new(),
            error: Some(e.to_string()),
        },
    }
}

pub(crate) fn errcode_nonzero(body: &str, field: &str, label: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = v.get(field)?.as_i64()?;
    if code != 0 {
        Some(format!("{label} {field}={code}: {body}"))
    } else {
        None
    }
}
