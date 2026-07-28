//! Telegram Bot API `sendMessage` notifier.

use crate::channel_opts::{is_markdown, option};
use crate::{build_text_for_channel, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};

pub struct TelegramNotifier;

impl TelegramNotifier {
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
                report.error.unwrap_or_else(|| "telegram failed".into()),
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
                report.error.unwrap_or_else(|| "telegram failed".into()),
            ))
        }
    }

    pub async fn send_text_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> crate::NotifySendReport {
        let Some(chat_id) = option(channel, "chat_id") else {
            return crate::NotifySendReport {
                ok: false,
                kind: "telegram".into(),
                request_url: channel.url.clone(),
                request_body: serde_json::json!({ "text": content }),
                http_status: None,
                response_body: String::new(),
                error: Some("telegram 需要 options.chat_id".into()),
            };
        };

        let url = send_message_url(&channel.url);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": content,
            "disable_web_page_preview": true,
        });
        if is_markdown(channel) {
            // Telegram MarkdownV2 is strict; use HTML for better template friendliness.
            body["parse_mode"] = serde_json::json!("HTML");
        }

        crate::post_json_report(http, "telegram", url, body, |t| {
            let v: serde_json::Value = serde_json::from_str(t).ok()?;
            if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
                None
            } else {
                Some(format!(
                    "telegram err: {}",
                    v.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or(t)
                ))
            }
        })
        .await
    }
}

/// Accept either `https://api.telegram.org/botTOKEN` or full `.../sendMessage`.
fn send_message_url(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/sendMessage") {
        base.to_string()
    } else {
        format!("{base}/sendMessage")
    }
}
