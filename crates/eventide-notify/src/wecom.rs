//! WeCom (企业微信) group robot notifier.

use crate::{build_text, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};
use serde::Serialize;

pub struct WeComNotifier;

#[derive(Serialize)]
struct WeText {
    content: String,
}

#[derive(Serialize)]
struct WeBody {
    msgtype: &'static str,
    text: WeText,
}

impl WeComNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let body = WeBody {
            msgtype: "text",
            text: WeText {
                content: build_text(rule, event, transition),
            },
        };
        let resp = http.post(&channel.url).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(NotifyError::Channel(format!("wecom http {status}: {text}")));
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(code) = v.get("errcode").and_then(|c| c.as_i64()) {
                if code != 0 {
                    return Err(NotifyError::Channel(format!("wecom err: {text}")));
                }
            }
        }
        Ok(())
    }
}
