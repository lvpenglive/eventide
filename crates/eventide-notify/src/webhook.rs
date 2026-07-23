//! Generic JSON webhook notifier.

use crate::{build_text, transition_label, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};
use serde::Serialize;

pub struct WebhookNotifier;

#[derive(Serialize)]
struct WebhookPayload<'a> {
    transition: &'a str,
    rule_name: &'a str,
    rule_id: String,
    alert_id: String,
    fingerprint: &'a str,
    status: &'a str,
    severity: &'a str,
    value: Option<f64>,
    labels: &'a eventide_core::Labels,
    annotations: &'a eventide_core::Labels,
    text: String,
}

impl WebhookNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let payload = WebhookPayload {
            transition: transition_label(transition),
            rule_name: &rule.name,
            rule_id: rule.id.to_string(),
            alert_id: event.id.to_string(),
            fingerprint: &event.fingerprint,
            status: event.status.as_str(),
            severity: event.severity.as_str(),
            value: event.value,
            labels: &event.labels,
            annotations: &event.annotations,
            text: build_text(rule, event, transition),
        };

        let resp = http
            .post(&channel.url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        if !resp.status().is_success() {
            return Err(NotifyError::Channel(format!(
                "webhook status {}",
                resp.status()
            )));
        }
        Ok(())
    }
}
