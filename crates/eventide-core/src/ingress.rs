//! Push-alert ingress: normalize external payloads and apply status transitions.

use crate::fingerprint::alert_fingerprint;
use crate::models::{
    AlertEvent, AlertStatus, AlertTransition, IngressAlert, IngressRoute, Labels, Severity,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Apply a pushed alert status onto an existing (or new) event.
pub fn apply_ingress(
    route: &IngressRoute,
    incoming: &IngressAlert,
    existing: Option<AlertEvent>,
    now: DateTime<Utc>,
) -> (AlertEvent, AlertTransition) {
    let mut labels = incoming.labels.clone();
    labels
        .entry("source".into())
        .or_insert_with(|| format!("ingress:{}", route.kind.as_str()));

    let fingerprint = incoming
        .fingerprint
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| alert_fingerprint(route.id, &labels));

    // Prefix fingerprint with route id so routes never collide.
    let fingerprint = format!("{}:{}", route.id, fingerprint);

    let target = match incoming.status {
        AlertStatus::Firing | AlertStatus::Pending => AlertStatus::Firing,
        AlertStatus::Resolved => AlertStatus::Resolved,
    };

    match existing {
        None => {
            if target == AlertStatus::Resolved {
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: route.id,
                    fingerprint,
                    status: AlertStatus::Resolved,
                    severity: incoming.severity,
                    labels,
                    annotations: incoming.annotations.clone(),
                    value: incoming.value,
                    starts_at: incoming.starts_at.unwrap_or(now),
                    ends_at: Some(incoming.ends_at.unwrap_or(now)),
                    pending_since: None,
                    last_evaluated_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                };
                (event, AlertTransition::Unchanged)
            } else {
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: route.id,
                    fingerprint,
                    status: AlertStatus::Firing,
                    severity: incoming.severity,
                    labels,
                    annotations: incoming.annotations.clone(),
                    value: incoming.value,
                    starts_at: incoming.starts_at.unwrap_or(now),
                    ends_at: None,
                    pending_since: None,
                    last_evaluated_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                };
                (event, AlertTransition::BecameFiring)
            }
        }
        Some(mut event) => {
            event.labels = labels;
            event.annotations = incoming.annotations.clone();
            event.severity = incoming.severity;
            event.value = incoming.value.or(event.value);
            event.last_evaluated_at = now;
            event.fingerprint = fingerprint;

            let transition = match (event.status, target) {
                (AlertStatus::Firing, AlertStatus::Firing)
                | (AlertStatus::Pending, AlertStatus::Firing) => {
                    event.status = AlertStatus::Firing;
                    event.ends_at = None;
                    AlertTransition::Unchanged
                }
                (AlertStatus::Resolved, AlertStatus::Firing) => {
                    event.status = AlertStatus::Firing;
                    event.starts_at = incoming.starts_at.unwrap_or(now);
                    event.ends_at = None;
                    event.notified_firing = false;
                    event.notified_resolved = false;
                    AlertTransition::BecameFiring
                }
                (AlertStatus::Firing, AlertStatus::Resolved)
                | (AlertStatus::Pending, AlertStatus::Resolved) => {
                    event.status = AlertStatus::Resolved;
                    event.ends_at = Some(incoming.ends_at.unwrap_or(now));
                    AlertTransition::BecameResolved
                }
                (AlertStatus::Resolved, AlertStatus::Resolved) => AlertTransition::Unchanged,
                _ => AlertTransition::Unchanged,
            };
            (event, transition)
        }
    }
}

/// Build a display name for notifications from ingress labels.
pub fn ingress_alert_name(route: &IngressRoute, labels: &Labels) -> String {
    labels
        .get("alertname")
        .cloned()
        .unwrap_or_else(|| route.name.clone())
}

// ---------- Alertmanager webhook ----------

#[derive(Debug, Deserialize)]
pub struct AlertmanagerWebhook {
    #[serde(default)]
    pub alerts: Vec<AlertmanagerAlert>,
}

#[derive(Debug, Deserialize)]
pub struct AlertmanagerAlert {
    pub status: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default, rename = "startsAt")]
    pub starts_at: Option<String>,
    #[serde(default, rename = "endsAt")]
    pub ends_at: Option<String>,
}

pub fn parse_alertmanager(body: &AlertmanagerWebhook) -> Vec<IngressAlert> {
    body.alerts
        .iter()
        .filter_map(|a| {
            let status = match a.status.to_ascii_lowercase().as_str() {
                "firing" => AlertStatus::Firing,
                "resolved" => AlertStatus::Resolved,
                _ => return None,
            };
            let severity = a
                .labels
                .get("severity")
                .and_then(|s| Severity::parse(s))
                .unwrap_or(Severity::Warning);
            Some(IngressAlert {
                status,
                fingerprint: a.fingerprint.clone(),
                labels: a.labels.clone(),
                annotations: a.annotations.clone(),
                severity,
                value: None,
                starts_at: a.starts_at.as_deref().and_then(parse_rfc3339),
                ends_at: a.ends_at.as_deref().and_then(parse_rfc3339_ends),
            })
        })
        .collect()
}

fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn parse_rfc3339_ends(s: &str) -> Option<DateTime<Utc>> {
    // Alertmanager uses zero time for "not ended"
    if s.starts_with("0001-01-01") {
        return None;
    }
    parse_rfc3339(s)
}

// ---------- Generic webhook ----------

#[derive(Debug, Deserialize)]
pub struct GenericWebhook {
    #[serde(default)]
    pub alerts: Vec<GenericAlert>,
}

#[derive(Debug, Deserialize)]
pub struct GenericAlert {
    pub status: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ends_at: Option<DateTime<Utc>>,
}

pub fn parse_generic(body: &GenericWebhook) -> Vec<IngressAlert> {
    body.alerts
        .iter()
        .filter_map(|a| {
            let status = match a.status.to_ascii_lowercase().as_str() {
                "firing" | "firing_alert" | "open" => AlertStatus::Firing,
                "resolved" | "ok" | "closed" => AlertStatus::Resolved,
                _ => return None,
            };
            let severity = a
                .severity
                .as_deref()
                .and_then(Severity::parse)
                .or_else(|| a.labels.get("severity").and_then(|s| Severity::parse(s)))
                .unwrap_or(Severity::Warning);
            Some(IngressAlert {
                status,
                fingerprint: a.fingerprint.clone(),
                labels: a.labels.clone(),
                annotations: a.annotations.clone(),
                severity,
                value: a.value,
                starts_at: a.starts_at,
                ends_at: a.ends_at,
            })
        })
        .collect()
}

// ---------- Probe / 业务拨测告警（Jeecg probe-alert.log） ----------

/// Detect Jeecg-style probe alert: `eventType` = fire|recover + `resultFlag`.
pub fn looks_like_probe_alert(v: &serde_json::Value) -> bool {
    v.get("eventType")
        .and_then(|x| x.as_str())
        .map(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "fire" | "recover" | "firing" | "resolved"
            )
        })
        .unwrap_or(false)
}

/// Parse one probe-alert JSON object into an [`IngressAlert`].
pub fn parse_probe_alert(v: &serde_json::Value) -> Option<IngressAlert> {
    if !v.is_object() {
        return None;
    }

    let event_type = v
        .get("eventType")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let result_flag = v
        .get("resultFlag")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();

    let status = match event_type.as_str() {
        "fire" | "firing" => AlertStatus::Firing,
        "recover" | "resolved" | "ok" => AlertStatus::Resolved,
        _ => match result_flag.as_str() {
            "BAD" | "FAIL" | "ERROR" => AlertStatus::Firing,
            "GOOD" | "OK" | "SUCCESS" => AlertStatus::Resolved,
            _ => return None,
        },
    };

    let category = json_str(v, "alertCategory").unwrap_or_default();
    let ret_code = json_any_str(v, "retCode").unwrap_or_default();
    let severity = if category.eq_ignore_ascii_case("infra") || ret_code == "10001" {
        Severity::Critical
    } else {
        Severity::Warning
    };

    let alertname = json_str(v, "bizchainName")
        .or_else(|| json_str(v, "businessName"))
        .or_else(|| json_str(v, "applicationName"))
        .or_else(|| json_str(v, "bizname"))
        .unwrap_or_else(|| "probe-alert".into());

    let mut labels: Labels = BTreeMap::new();
    labels.insert("alertname".into(), alertname);
    for key in [
        "bizname",
        "areacode",
        "applicationId",
        "applicationName",
        "bizchainId",
        "bizchainName",
        "businessId",
        "businessName",
        "alertCategory",
        "alertIp",
        "ipaddr",
        "resultFlag",
        "retCode",
        "resultId",
    ] {
        if let Some(s) = json_any_str(v, key) {
            labels.insert(key.into(), s);
        }
    }
    if let Some(port) = json_any_str(v, "bizPort") {
        labels.insert("bizPort".into(), port);
    }

    let mut annotations: Labels = BTreeMap::new();
    if let Some(msg) = json_str(v, "retMessage") {
        annotations.insert("summary".into(), truncate(&msg, 500));
        annotations.insert("retMessage".into(), truncate(&msg, 2000));
    }
    if let Some(t) = json_any_str(v, "retTimeMs") {
        annotations.insert("retTimeMs".into(), t);
    }
    if let Some(et) = json_str(v, "eventTime") {
        annotations.insert("eventTime".into(), et);
    }

    let fingerprint = json_any_str(v, "messageId").or_else(|| json_any_str(v, "resultId"));

    let value = v
        .get("retTimeMs")
        .and_then(|x| {
            x.as_f64()
                .or_else(|| x.as_i64().map(|n| n as f64))
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        });

    let starts_at = json_str(v, "eventTime").and_then(|s| parse_probe_time(&s));

    Some(IngressAlert {
        status,
        fingerprint,
        labels,
        annotations,
        severity,
        value,
        starts_at,
        ends_at: None,
    })
}

/// Extract JSON object/array substring from a log line (e.g. `INFO alertLogger - {...}`).
pub fn extract_json_payload(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(i) = trimmed.find('{') {
        if let Some(j) = trimmed.rfind('}') {
            if j >= i {
                return &trimmed[i..=j];
            }
        }
    }
    if let Some(i) = trimmed.find('[') {
        if let Some(j) = trimmed.rfind(']') {
            if j >= i {
                return &trimmed[i..=j];
            }
        }
    }
    trimmed
}

/// Auto-detect Alertmanager / Generic / Probe payloads (raw bytes or log lines).
pub fn parse_ingress_payload(raw: &[u8]) -> Result<Vec<IngressAlert>, String> {
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let json_text = extract_json_payload(text);

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_text) {
        if looks_like_probe_alert(&v) {
            return parse_probe_alert(&v)
                .map(|a| vec![a])
                .ok_or_else(|| "probe alert could not be mapped".into());
        }
        if let Some(arr) = v.as_array() {
            if arr.iter().any(looks_like_probe_alert) {
                let alerts: Vec<_> = arr.iter().filter_map(parse_probe_alert).collect();
                if !alerts.is_empty() {
                    return Ok(alerts);
                }
            }
        }
        // Generic wrap: { "alerts": [ probe, ... ] }
        if let Some(arr) = v.get("alerts").and_then(|a| a.as_array()) {
            if arr.iter().any(looks_like_probe_alert) {
                let alerts: Vec<_> = arr.iter().filter_map(parse_probe_alert).collect();
                if !alerts.is_empty() {
                    return Ok(alerts);
                }
            }
        }
    }

    if let Ok(w) = serde_json::from_str::<AlertmanagerWebhook>(json_text) {
        let alerts = parse_alertmanager(&w);
        if !alerts.is_empty() || json_text.contains("\"alerts\"") {
            return Ok(alerts);
        }
    }
    if let Ok(w) = serde_json::from_str::<GenericWebhook>(json_text) {
        let alerts = parse_generic(&w);
        if !alerts.is_empty() {
            return Ok(alerts);
        }
    }
    if let Ok(one) = serde_json::from_str::<GenericAlert>(json_text) {
        return Ok(parse_generic(&GenericWebhook { alerts: vec![one] }));
    }

    Err("payload is not alertmanager/generic/probe JSON".into())
}

fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn json_any_str(v: &serde_json::Value, key: &str) -> Option<String> {
    let x = v.get(key)?;
    match x {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

fn parse_probe_time(s: &str) -> Option<DateTime<Utc>> {
    if let Some(dt) = parse_rfc3339(s) {
        return Some(dt);
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::IngressKind;

    fn route() -> IngressRoute {
        IngressRoute {
            id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            name: "am".into(),
            kind: IngressKind::Alertmanager,
            token: None,
            endpoint: String::new(),
            options: BTreeMap::new(),
            channel_ids: vec![],
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn ingress_new_firing() {
        let mut labels = BTreeMap::new();
        labels.insert("alertname".into(), "HighCPU".into());
        let incoming = IngressAlert {
            status: AlertStatus::Firing,
            fingerprint: Some("fp1".into()),
            labels,
            annotations: BTreeMap::new(),
            severity: Severity::Critical,
            value: Some(99.0),
            starts_at: None,
            ends_at: None,
        };
        let (ev, t) = apply_ingress(&route(), &incoming, None, Utc::now());
        assert_eq!(ev.status, AlertStatus::Firing);
        assert_eq!(t, AlertTransition::BecameFiring);
        assert!(ev.fingerprint.contains("fp1"));
    }

    #[test]
    fn ingress_resolve_edge() {
        let r = route();
        let mut labels = BTreeMap::new();
        labels.insert("alertname".into(), "HighCPU".into());
        let firing = IngressAlert {
            status: AlertStatus::Firing,
            fingerprint: Some("fp1".into()),
            labels: labels.clone(),
            annotations: BTreeMap::new(),
            severity: Severity::Warning,
            value: None,
            starts_at: None,
            ends_at: None,
        };
        let (ev, _) = apply_ingress(&r, &firing, None, Utc::now());
        let resolved = IngressAlert {
            status: AlertStatus::Resolved,
            fingerprint: Some("fp1".into()),
            labels,
            annotations: BTreeMap::new(),
            severity: Severity::Warning,
            value: None,
            starts_at: None,
            ends_at: None,
        };
        let (ev2, t) = apply_ingress(&r, &resolved, Some(ev), Utc::now());
        assert_eq!(ev2.status, AlertStatus::Resolved);
        assert_eq!(t, AlertTransition::BecameResolved);
    }

    #[test]
    fn parse_probe_fire_and_recover() {
        let fire = r#"{
            "eventType":"fire","eventTime":"2026-07-21 15:18:39",
            "alertIp":"127.0.0.1","bizPort":9000,"bizname":"oaec",
            "messageId":"2077682656462446593","resultFlag":"BAD","retCode":"4001",
            "retMessage":"连接拒绝","retTimeMs":12976,"areacode":"jyq",
            "applicationName":"办公管理系统","bizchainName":"业务拨测-办公管理系统",
            "alertCategory":"business"
        }"#;
        let alerts = parse_ingress_payload(fire.as_bytes()).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].status, AlertStatus::Firing);
        assert_eq!(
            alerts[0].fingerprint.as_deref(),
            Some("2077682656462446593")
        );
        assert_eq!(
            alerts[0].labels.get("alertname").map(|s| s.as_str()),
            Some("业务拨测-办公管理系统")
        );
        assert_eq!(alerts[0].value, Some(12976.0));

        let recover = r#"2026-07-21 17:00:29.175 [Worker] INFO  alertLogger - {"eventType":"recover","eventTime":"2026-07-21 17:00:28","messageId":"2077682656462446593","resultFlag":"GOOD","retCode":"4000","retMessage":"成功","retTimeMs":26010,"bizchainName":"业务拨测-办公管理系统","alertCategory":"business"}"#;
        let alerts = parse_ingress_payload(recover.as_bytes()).unwrap();
        assert_eq!(alerts[0].status, AlertStatus::Resolved);
        assert_eq!(
            alerts[0].fingerprint.as_deref(),
            Some("2077682656462446593")
        );
    }

    #[test]
    fn parse_probe_infra_critical() {
        let raw = r#"{"eventType":"fire","resultFlag":"BAD","retCode":"10001","retMessage":"代理失败","alertCategory":"infra","messageId":"m1","bizchainName":"链"}"#;
        let a = parse_probe_alert(&serde_json::from_str(raw).unwrap()).unwrap();
        assert_eq!(a.severity, Severity::Critical);
    }

    #[test]
    fn parse_am_payload() {
        let raw = r#"{
            "alerts": [{
                "status": "firing",
                "labels": {"alertname": "Test", "severity": "critical"},
                "annotations": {"summary": "x"},
                "fingerprint": "abc",
                "startsAt": "2024-01-01T00:00:00Z",
                "endsAt": "0001-01-01T00:00:00Z"
            }]
        }"#;
        let body: AlertmanagerWebhook = serde_json::from_str(raw).unwrap();
        let alerts = parse_alertmanager(&body);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].status, AlertStatus::Firing);
        assert_eq!(alerts[0].severity, Severity::Critical);
        assert!(alerts[0].ends_at.is_none());
    }
}
