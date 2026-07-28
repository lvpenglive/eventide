//! Threshold comparison and alert state machine.

use crate::fingerprint::alert_fingerprint;
use crate::models::{
    AlertEvent, AlertStatus, AlertTransition, Comparator, Labels, Rule,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// One metric sample from a datasource query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub labels: Labels,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
}

/// Result of evaluating one sample against a rule (pre state-machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluateResult {
    /// Threshold condition is true.
    Matching,
    /// Threshold condition is false.
    NotMatching,
}

/// Compare `value` against `threshold` using `op`.
pub fn compare(op: Comparator, value: f64, threshold: f64) -> bool {
    match op {
        Comparator::Gt => value > threshold,
        Comparator::Gte => value >= threshold,
        Comparator::Lt => value < threshold,
        Comparator::Lte => value <= threshold,
        Comparator::Eq => (value - threshold).abs() < f64::EPSILON,
        Comparator::Neq => (value - threshold).abs() >= f64::EPSILON,
    }
}

pub fn evaluate_sample(rule: &Rule, sample: &MetricSample) -> EvaluateResult {
    if compare(rule.comparator, sample.value, rule.threshold) {
        EvaluateResult::Matching
    } else {
        EvaluateResult::NotMatching
    }
}

/// Merge rule labels with sample labels (sample wins on key conflict).
pub fn merge_labels(rule: &Rule, sample: &MetricSample) -> Labels {
    let mut labels = rule.labels.clone();
    for (k, v) in &sample.labels {
        labels.insert(k.clone(), v.clone());
    }
    labels
}

/// Apply evaluation result to an existing (or new) alert event.
///
/// Returns the updated event and whether a notify edge occurred.
pub fn apply_evaluation(
    rule: &Rule,
    sample: &MetricSample,
    existing: Option<AlertEvent>,
    now: DateTime<Utc>,
) -> (AlertEvent, AlertTransition) {
    let labels = merge_labels(rule, sample);
    let fingerprint = alert_fingerprint(rule.id, &labels);
    let matching = evaluate_sample(rule, sample) == EvaluateResult::Matching;
    let for_dur = Duration::seconds(rule.for_seconds as i64);

    match existing {
        None => {
            if !matching {
                // No prior event and not matching — synthesize a resolved placeholder
                // that callers typically skip persisting.
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: rule.id,
                    fingerprint,
                    status: AlertStatus::Resolved,
                    severity: rule.severity,
                    labels,
                    annotations: rule.annotations.clone(),
                    value: Some(sample.value),
                    starts_at: now,
                    ends_at: Some(now),
                    pending_since: None,
                    last_evaluated_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                };
                (event, AlertTransition::Unchanged)
            } else if rule.for_seconds == 0 {
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: rule.id,
                    fingerprint,
                    status: AlertStatus::Firing,
                    severity: rule.severity,
                    labels,
                    annotations: rule.annotations.clone(),
                    value: Some(sample.value),
                    starts_at: now,
                    ends_at: None,
                    pending_since: None,
                    last_evaluated_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                };
                (event, AlertTransition::BecameFiring)
            } else {
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: rule.id,
                    fingerprint,
                    status: AlertStatus::Pending,
                    severity: rule.severity,
                    labels,
                    annotations: rule.annotations.clone(),
                    value: Some(sample.value),
                    starts_at: now,
                    ends_at: None,
                    pending_since: Some(now),
                    last_evaluated_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                };
                (event, AlertTransition::Unchanged)
            }
        }
        Some(mut event) => {
            event.value = Some(sample.value);
            event.last_evaluated_at = now;
            event.labels = labels;
            event.annotations = rule.annotations.clone();
            event.severity = rule.severity;

            let transition = match (event.status, matching) {
                (AlertStatus::Pending, true) => {
                    let since = event.pending_since.unwrap_or(event.starts_at);
                    if now - since >= for_dur {
                        event.status = AlertStatus::Firing;
                        event.pending_since = None;
                        event.starts_at = now;
                        event.ends_at = None;
                        AlertTransition::BecameFiring
                    } else {
                        AlertTransition::Unchanged
                    }
                }
                (AlertStatus::Pending, false) => {
                    event.status = AlertStatus::Resolved;
                    event.ends_at = Some(now);
                    event.pending_since = None;
                    AlertTransition::Unchanged
                }
                (AlertStatus::Firing, true) => AlertTransition::Unchanged,
                (AlertStatus::Firing, false) => {
                    event.status = AlertStatus::Resolved;
                    event.ends_at = Some(now);
                    AlertTransition::BecameResolved
                }
                (AlertStatus::Resolved, true) => {
                    if rule.for_seconds == 0 {
                        event.status = AlertStatus::Firing;
                        event.starts_at = now;
                        event.ends_at = None;
                        event.pending_since = None;
                        event.notified_firing = false;
                        event.notified_resolved = false;
                        AlertTransition::BecameFiring
                    } else {
                        event.status = AlertStatus::Pending;
                        event.starts_at = now;
                        event.ends_at = None;
                        event.pending_since = Some(now);
                        event.notified_firing = false;
                        event.notified_resolved = false;
                        AlertTransition::Unchanged
                    }
                }
                (AlertStatus::Resolved, false) => AlertTransition::Unchanged,
            };

            // Keep fingerprint stable for the event identity.
            event.fingerprint = fingerprint;
            (event, transition)
        }
    }
}

/// Convenience: evaluate all samples for a rule against a map of existing events by fingerprint.
pub fn evaluate_rule(
    rule: &Rule,
    samples: &[MetricSample],
    existing_by_fp: &BTreeMap<String, AlertEvent>,
    now: DateTime<Utc>,
) -> Vec<(AlertEvent, AlertTransition)> {
    let mut results = Vec::with_capacity(samples.len());
    let mut seen = std::collections::HashSet::new();

    for sample in samples {
        let labels = merge_labels(rule, sample);
        let fp = alert_fingerprint(rule.id, &labels);
        seen.insert(fp.clone());
        let existing = existing_by_fp.get(&fp).cloned();
        results.push(apply_evaluation(rule, sample, existing, now));
    }

    // Samples that disappeared: resolve any still-firing/pending events.
    for (fp, event) in existing_by_fp {
        if seen.contains(fp) {
            continue;
        }
        if matches!(event.status, AlertStatus::Firing | AlertStatus::Pending) {
            results.push(resolve_missing(event.clone(), now));
        }
    }

    results
}

fn resolve_missing(mut event: AlertEvent, now: DateTime<Utc>) -> (AlertEvent, AlertTransition) {
    event.last_evaluated_at = now;
    match event.status {
        AlertStatus::Firing => {
            event.status = AlertStatus::Resolved;
            event.ends_at = Some(now);
            (event, AlertTransition::BecameResolved)
        }
        AlertStatus::Pending => {
            event.status = AlertStatus::Resolved;
            event.ends_at = Some(now);
            event.pending_since = None;
            (event, AlertTransition::Unchanged)
        }
        AlertStatus::Resolved => (event, AlertTransition::Unchanged),
    }
}

/// Build a human-readable alert title.
pub fn alert_title(rule: &Rule, event: &AlertEvent) -> String {
    format!(
        "[{}] {} {} {} (value={:?})",
        event.severity.as_str(),
        rule.name,
        rule.comparator.as_str(),
        rule.threshold,
        event.value
    )
}

/// Plain-text body used by notification channels (and stored in notify_logs).
pub fn format_notify_text(rule: &Rule, event: &AlertEvent, transition: AlertTransition) -> String {
    let status = match transition {
        AlertTransition::BecameFiring => "firing",
        AlertTransition::BecameResolved => "resolved",
        AlertTransition::Unchanged => "unchanged",
    };
    format!(
        "{}\nstatus: {}\nfingerprint: {}\nlabels: {}\n",
        alert_title(rule, event),
        status,
        event.fingerprint,
        serde_json::to_string(&event.labels).unwrap_or_else(|_| "{}".into())
    )
}

/// Render notify body using an optional channel template; empty → default text.
pub fn format_notify_body(
    template: Option<&str>,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> String {
    let Some(tpl) = template.map(str::trim).filter(|s| !s.is_empty()) else {
        return format_notify_text(rule, event, transition);
    };
    let title = alert_title(rule, event);
    let transition_label = match transition {
        AlertTransition::BecameFiring => "firing",
        AlertTransition::BecameResolved => "resolved",
        AlertTransition::Unchanged => "unchanged",
    };
    let labels_json = serde_json::to_string(&event.labels).unwrap_or_else(|_| "{}".into());
    let annotations_json =
        serde_json::to_string(&event.annotations).unwrap_or_else(|_| "{}".into());
    let ctx = crate::template::TemplateContext {
        labels: &event.labels,
        annotations: &event.annotations,
        value: event.value,
        severity: event.severity.as_str(),
        status: event.status.as_str(),
        fingerprint: &event.fingerprint,
        rule_name: Some(rule.name.as_str()),
        title: Some(title.as_str()),
        transition: Some(transition_label),
        labels_json: Some(labels_json.as_str()),
        annotations_json: Some(annotations_json.as_str()),
    };
    crate::template::render_template(tpl, &ctx)
}

/// Channel-aware notify body (uses `template_firing` / `template_resolved` / `template`).
pub fn format_notify_text_for_channel(
    channel: &crate::models::NotifyChannel,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> String {
    format_notify_body(channel.notify_template(transition), rule, event, transition)
}

fn notify_template_ctx<'a>(
    rule: &'a Rule,
    event: &'a AlertEvent,
    title: &'a str,
    transition_label: &'a str,
    labels_json: &'a str,
    annotations_json: &'a str,
) -> crate::template::TemplateContext<'a> {
    crate::template::TemplateContext {
        labels: &event.labels,
        annotations: &event.annotations,
        value: event.value,
        severity: event.severity.as_str(),
        status: event.status.as_str(),
        fingerprint: &event.fingerprint,
        rule_name: Some(rule.name.as_str()),
        title: Some(title),
        transition: Some(transition_label),
        labels_json: Some(labels_json),
        annotations_json: Some(annotations_json),
    }
}

fn transition_str(transition: AlertTransition) -> &'static str {
    match transition {
        AlertTransition::BecameFiring => "firing",
        AlertTransition::BecameResolved => "resolved",
        AlertTransition::Unchanged => "unchanged",
    }
}

/// Default JSON payload for custom HTTP channel when no template is set.
pub fn default_http_json_payload(
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> serde_json::Value {
    serde_json::json!({
        "transition": transition_str(transition),
        "rule_name": rule.name,
        "rule_id": rule.id.to_string(),
        "alert_id": event.id.to_string(),
        "fingerprint": event.fingerprint,
        "status": event.status.as_str(),
        "severity": event.severity.as_str(),
        "value": event.value,
        "labels": event.labels,
        "annotations": event.annotations,
        "text": format_notify_text(rule, event, transition),
    })
}

/// Render custom HTTP JSON body from template; empty template → default payload.
pub fn format_notify_json(
    template: Option<&str>,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> Result<serde_json::Value, String> {
    let Some(tpl) = template.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(default_http_json_payload(rule, event, transition));
    };
    let title = alert_title(rule, event);
    let transition_label = transition_str(transition);
    let labels_json = serde_json::to_string(&event.labels).unwrap_or_else(|_| "{}".into());
    let annotations_json =
        serde_json::to_string(&event.annotations).unwrap_or_else(|_| "{}".into());
    let ctx = notify_template_ctx(
        rule,
        event,
        &title,
        transition_label,
        &labels_json,
        &annotations_json,
    );
    let rendered = crate::template::render_template(tpl, &ctx);
    serde_json::from_str(&rendered).map_err(|e| {
        format!("自定义 JSON 模板解析失败: {e}；渲染结果前 200 字: {}", {
            let s: String = rendered.chars().take(200).collect();
            s
        })
    })
}

pub fn format_notify_json_for_channel(
    channel: &crate::models::NotifyChannel,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> Result<serde_json::Value, String> {
    format_notify_json(channel.json_template(transition), rule, event, transition)
}

/// Body stored in notify_logs (text or JSON string).
pub fn format_notify_log_body(
    channel: &crate::models::NotifyChannel,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
) -> String {
    if channel.kind == crate::models::ChannelKind::Http {
        match format_notify_json_for_channel(channel, rule, event, transition) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
            Err(e) => format!("(json template error) {e}"),
        }
    } else {
        format_notify_text_for_channel(channel, rule, event, transition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Comparator, Severity};

    fn sample_rule(for_seconds: u64) -> Rule {
        Rule {
            id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            name: "high_cpu".into(),
            datasource_id: Uuid::nil(),
            expr: "cpu".into(),
            comparator: Comparator::Gt,
            threshold: 80.0,
            for_seconds,
            interval_seconds: 30,
            severity: Severity::Critical,
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
            channel_ids: vec![],
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample(value: f64) -> MetricSample {
        let mut labels = BTreeMap::new();
        labels.insert("instance".into(), "a".into());
        MetricSample {
            labels,
            value,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn compare_ops() {
        assert!(compare(Comparator::Gt, 81.0, 80.0));
        assert!(!compare(Comparator::Gt, 80.0, 80.0));
        assert!(compare(Comparator::Gte, 80.0, 80.0));
        assert!(compare(Comparator::Lt, 1.0, 2.0));
        assert!(compare(Comparator::Neq, 1.0, 2.0));
    }

    #[test]
    fn immediate_firing_when_for_zero() {
        let rule = sample_rule(0);
        let now = Utc::now();
        let (event, t) = apply_evaluation(&rule, &sample(90.0), None, now);
        assert_eq!(event.status, AlertStatus::Firing);
        assert_eq!(t, AlertTransition::BecameFiring);
    }

    #[test]
    fn pending_then_firing_after_for() {
        let rule = sample_rule(60);
        let t0 = Utc::now();
        let (pending, t1) = apply_evaluation(&rule, &sample(90.0), None, t0);
        assert_eq!(pending.status, AlertStatus::Pending);
        assert_eq!(t1, AlertTransition::Unchanged);

        let t1_time = t0 + Duration::seconds(61);
        let (firing, t2) = apply_evaluation(&rule, &sample(91.0), Some(pending), t1_time);
        assert_eq!(firing.status, AlertStatus::Firing);
        assert_eq!(t2, AlertTransition::BecameFiring);
    }

    #[test]
    fn firing_to_resolved() {
        let rule = sample_rule(0);
        let now = Utc::now();
        let (firing, _) = apply_evaluation(&rule, &sample(90.0), None, now);
        let (resolved, t) = apply_evaluation(&rule, &sample(10.0), Some(firing), now);
        assert_eq!(resolved.status, AlertStatus::Resolved);
        assert_eq!(t, AlertTransition::BecameResolved);
    }

    #[test]
    fn notify_template_renders_custom_body() {
        let rule = sample_rule(0);
        let now = Utc::now();
        let (event, _) = apply_evaluation(&rule, &sample(90.0), None, now);
        let body = format_notify_body(
            Some("[{{severity}}] {{rule.name}} val={{value}} edge={{transition}}"),
            &rule,
            &event,
            AlertTransition::BecameFiring,
        );
        assert!(body.contains("[critical]"));
        assert!(body.contains("high_cpu"));
        assert!(body.contains("val=90"));
        assert!(body.contains("edge=firing"));
    }

    #[test]
    fn notify_template_empty_falls_back() {
        let rule = sample_rule(0);
        let now = Utc::now();
        let (event, _) = apply_evaluation(&rule, &sample(90.0), None, now);
        let body = format_notify_body(Some("  "), &rule, &event, AlertTransition::BecameFiring);
        let default = format_notify_text(&rule, &event, AlertTransition::BecameFiring);
        assert_eq!(body, default);
    }

    #[test]
    fn notify_json_template_and_json_escape() {
        let rule = sample_rule(0);
        let now = Utc::now();
        let (mut event, _) = apply_evaluation(&rule, &sample(90.0), None, now);
        event
            .annotations
            .insert("summary".into(), r#"disk "C:" full"#.into());
        let v = format_notify_json(
            Some(r#"{"msg": {{annotations.summary|json}}, "sev": {{severity|json}}}"#),
            &rule,
            &event,
            AlertTransition::BecameFiring,
        )
        .expect("json ok");
        assert_eq!(v["msg"], "disk \"C:\" full");
        assert_eq!(v["sev"], "critical");
    }
}
