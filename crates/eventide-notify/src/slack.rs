//! Slack Incoming Webhook notifier.

use crate::channel_opts::is_markdown;
use crate::{build_text_for_channel, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};

pub struct SlackNotifier;

impl SlackNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let content = build_text_for_channel(channel, rule, event, transition);
        let report = Self::send_text_report(http, channel, &content).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report.error.unwrap_or_else(|| "slack failed".into()),
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
                report.error.unwrap_or_else(|| "slack failed".into()),
            ))
        }
    }

    pub async fn send_text_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> crate::NotifySendReport {
        let body = if is_markdown(channel) {
            // mrkdwn section block
            serde_json::json!({
                "text": content,
                "blocks": [{
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": content
                    }
                }]
            })
        } else {
            serde_json::json!({ "text": content })
        };
        crate::post_json_report(http, "slack", channel.url.clone(), body, |t| {
            // Slack returns "ok" plain text on success for incoming webhooks.
            let trimmed = t.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("ok") {
                None
            } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                if v.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                    Some(format!("slack err: {t}"))
                } else {
                    None
                }
            } else if trimmed.eq_ignore_ascii_case("ok") {
                None
            } else {
                // Non-JSON non-ok body often means error string.
                Some(format!("slack err: {t}"))
            }
        })
        .await
    }
}
