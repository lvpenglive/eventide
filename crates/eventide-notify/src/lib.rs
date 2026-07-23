//! Notification channels: Webhook, DingTalk, WeCom, Feishu.

mod dingtalk;
mod feishu;
mod webhook;
mod wecom;

use eventide_core::{AlertEvent, AlertTransition, ChannelKind, NotifyChannel, Rule};
use thiserror::Error;

pub use dingtalk::DingTalkNotifier;
pub use feishu::FeishuNotifier;
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
        AlertTransition::Unchanged => "unchanged",
    }
}

pub(crate) fn build_text(rule: &Rule, event: &AlertEvent, transition: AlertTransition) -> String {
    let title = eventide_core::alert_title(rule, event);
    format!(
        "{}\nstatus: {}\nfingerprint: {}\nlabels: {}\n",
        title,
        transition_label(transition),
        event.fingerprint,
        serde_json::to_string(&event.labels).unwrap_or_else(|_| "{}".into())
    )
}
