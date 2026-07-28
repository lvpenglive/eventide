//! WeCom (企业微信) group robot notifier (text / markdown + @).

use crate::channel_opts::{at_all, at_mobiles, is_markdown};
use crate::{build_text_for_channel, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};

pub struct WeComNotifier;

impl WeComNotifier {
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
                report.error.unwrap_or_else(|| "wecom failed".into()),
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
                report.error.unwrap_or_else(|| "wecom failed".into()),
            ))
        }
    }

    pub async fn send_text_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> crate::NotifySendReport {
        let body = build_body(channel, content);
        crate::post_json_report(http, "wecom", channel.url.clone(), body, |t| {
            crate::errcode_nonzero(t, "errcode", "wecom")
        })
        .await
    }
}

fn build_body(channel: &NotifyChannel, content: &str) -> serde_json::Value {
    if is_markdown(channel) {
        // Markdown 模式：企微 markdown 不支持 mentioned_*，可在正文写 `<@userid>` / `@all`。
        serde_json::json!({
            "msgtype": "markdown",
            "markdown": { "content": content },
        })
    } else {
        let mobiles = at_mobiles(channel);
        serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
                "mentioned_list": if at_all(channel) { vec!["@all".to_string()] } else { Vec::<String>::new() },
                "mentioned_mobile_list": mobiles,
            }
        })
    }
}
