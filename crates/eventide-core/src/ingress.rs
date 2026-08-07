//! Push-alert ingress: normalize external payloads and apply status transitions.

use crate::fingerprint::alert_fingerprint;
use crate::models::{
    AlertEvent, AlertStatus, AlertTransition, IngressAlert, IngressRoute, Labels, Severity,
};
use crate::template::{apply_string_transform, split_path_transform};
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
                    tally: 1,
                    last_occurrence_at: now,
                    notified_firing: false,
                    notified_resolved: false,
                    acknowledged_at: None,
                    acknowledged_by: None,
                    assignee: None,
                    ack_comment: None,
                    closed_at: None,
                    closed_by: None,
                    close_comment: None,
                    escalated_at: None,
                };
                (event, AlertTransition::Unchanged)
            } else {
                let starts = incoming.starts_at.unwrap_or(now);
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: route.id,
                    fingerprint,
                    status: AlertStatus::Firing,
                    severity: incoming.severity,
                    labels,
                    annotations: incoming.annotations.clone(),
                    value: incoming.value,
                    starts_at: starts,
                    ends_at: None,
                    pending_since: None,
                    last_evaluated_at: now,
                    tally: 1,
                    last_occurrence_at: starts,
                    notified_firing: false,
                    notified_resolved: false,
                    acknowledged_at: None,
                    acknowledged_by: None,
                    assignee: None,
                    ack_comment: None,
                    closed_at: None,
                    closed_by: None,
                    close_comment: None,
                    escalated_at: None,
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
                    event.bump_occurrence(now);
                    AlertTransition::Unchanged
                }
                (AlertStatus::Resolved, AlertStatus::Firing) => {
                    event.status = AlertStatus::Firing;
                    event.starts_at = incoming.starts_at.unwrap_or(now);
                    event.ends_at = None;
                    event.notified_firing = false;
                    event.notified_resolved = false;
                    event.clear_ack();
                    event.clear_close();
                    event.clear_escalation();
                    event.reset_occurrence(now);
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
    #[serde(default, alias = "startsAt")]
    pub starts_at: Option<DateTime<Utc>>,
    #[serde(default, alias = "endsAt")]
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

// ---------- Probe / ???????Jeecg probe-alert.log? ----------

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
        Severity::Disaster
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
    // Keep ip / alertIp / instance aligned so console "告警 IP" and enrich lookups work.
    let ip = labels
        .get("alertIp")
        .cloned()
        .or_else(|| labels.get("ipaddr").cloned())
        .or_else(|| json_any_str(v, "ip"))
        .filter(|s| !s.is_empty());
    if let Some(ip) = ip {
        labels.insert("ip".into(), ip.clone());
        labels.entry("alertIp".to_string()).or_insert_with(|| ip.clone());
        labels.entry("instance".to_string()).or_insert(ip);
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

/// Field mapping from Ingress `options` (JSON dotted paths).
///
/// Enable by filling any `map_*` path, or set `map_enabled=1`.
#[derive(Debug, Clone, Default)]
pub struct FieldMapping {
    pub enabled: bool,
    /// Path to array of alert objects (e.g. `alerts` / `data.items`). Empty = root object or root array.
    pub list: String,
    pub status: String,
    pub fire_values: Vec<String>,
    pub resolve_values: Vec<String>,
    pub name: String,
    pub description: String,
    pub ip: String,
    pub value: String,
    pub fingerprint: String,
    pub severity: String,
    pub critical_values: Vec<String>,
    pub warning_values: Vec<String>,
    /// Extra labels: `key:path,key2:path2`
    pub labels: Vec<(String, String)>,
}

impl FieldMapping {
    pub fn from_options(options: &BTreeMap<String, String>) -> Self {
        let get = |k: &str| options.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
        let list = get("map_list");
        let status = get("map_status");
        let name = get("map_name");
        let description = get("map_description");
        let ip = get("map_ip");
        let value = get("map_value");
        let fingerprint = get("map_fingerprint");
        let severity = get("map_severity");
        let labels_raw = get("map_labels");
        let enabled_flag = get("map_enabled").to_ascii_lowercase();
        let has_paths = !status.is_empty()
            || !name.is_empty()
            || !description.is_empty()
            || !ip.is_empty()
            || !value.is_empty()
            || !fingerprint.is_empty()
            || !severity.is_empty()
            || !list.is_empty()
            || !labels_raw.is_empty();
        let enabled = match enabled_flag.as_str() {
            "0" | "false" | "no" | "off" => false,
            "1" | "true" | "yes" | "on" => true,
            _ => has_paths,
        };
        let fire_values = split_csv(
            &get("map_fire"),
            &["fire", "firing", "open", "bad", "fail", "error", "1", "true", "critical"],
        );
        let resolve_values = split_csv(
            &get("map_resolve"),
            &[
                "recover", "resolved", "ok", "closed", "good", "success", "0", "false",
            ],
        );
        let critical_values = split_csv(
            &get("map_critical"),
            &[
                "disaster",
                "critical",
                "crit",
                "p1",
                "fatal",
                "high",
                "5",
                "4",
            ],
        );
        let warning_values = split_csv(
            &get("map_warning"),
            &["warning", "warn", "average", "p2", "p3", "2", "3"],
        );

        let labels = labels_raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|pair| {
                let (k, p) = pair.split_once(':')?;
                let k = k.trim();
                let p = p.trim();
                if k.is_empty() || p.is_empty() {
                    None
                } else {
                    Some((k.to_string(), p.to_string()))
                }
            })
            .collect();

        Self {
            enabled,
            list,
            status,
            fire_values,
            resolve_values,
            name,
            description,
            ip,
            value,
            fingerprint,
            severity,
            critical_values,
            warning_values,
            labels,
        }
    }
}

fn split_csv(raw: &str, defaults: &[&str]) -> Vec<String> {
    let parts: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        defaults.iter().map(|s| (*s).to_string()).collect()
    } else {
        parts
    }
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_start_matches("$.")
        .trim_start_matches('$')
        .trim_start_matches('.')
        .to_string()
}

fn json_path_value<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let path = normalize_path(path);
    if path.is_empty() {
        return Some(root);
    }
    let mut cur = root;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        // support simple array index: items[0]
        if let Some((name, idx_s)) = part.split_once('[') {
            if !name.is_empty() {
                cur = cur.get(name)?;
            }
            let idx: usize = idx_s.trim_end_matches(']').parse().ok()?;
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(part)?;
        }
    }
    Some(cur)
}

fn path_as_string(root: &serde_json::Value, path: &str) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }
    let (json_path, transform) = split_path_transform(path);
    let v = json_path_value(root, &json_path)?;
    let raw = match v {
        serde_json::Value::String(s) if !s.is_empty() => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => return None,
        other => {
            let s = other.to_string();
            if s == "null" || s.is_empty() {
                return None;
            }
            s.trim_matches('"').to_string()
        }
    };
    Some(apply_string_transform(&raw, transform.as_deref()))
}

fn path_as_f64(root: &serde_json::Value, path: &str) -> Option<f64> {
    if path.trim().is_empty() {
        return None;
    }
    let v = json_path_value(root, path)?;
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn value_in_list(val: &str, list: &[String]) -> bool {
    let v = val.trim().to_ascii_lowercase();
    list.iter().any(|x| x == &v)
}

/// Map one JSON object using [`FieldMapping`].
pub fn parse_mapped_alert(v: &serde_json::Value, m: &FieldMapping) -> Option<IngressAlert> {
    if !v.is_object() && !v.is_array() {
        // allow primitives? no
    }
    if !v.is_object() {
        return None;
    }

    let status_raw = if m.status.is_empty() {
        String::new()
    } else {
        path_as_string(v, &m.status).unwrap_or_default()
    };
    let status = if !status_raw.is_empty() {
        if value_in_list(&status_raw, &m.fire_values) {
            AlertStatus::Firing
        } else if value_in_list(&status_raw, &m.resolve_values) {
            AlertStatus::Resolved
        } else {
            // unknown status string ? treat non-empty non-resolve as firing if looks bad
            return None;
        }
    } else {
        // no status path: default firing (push implies alert)
        AlertStatus::Firing
    };

    let name = path_as_string(v, &m.name).unwrap_or_else(|| "mapped-alert".into());
    let description = path_as_string(v, &m.description).unwrap_or_default();
    let ip = path_as_string(v, &m.ip).unwrap_or_default();
    let fingerprint = path_as_string(v, &m.fingerprint);
    let value = path_as_f64(v, &m.value);

    let sev_raw = path_as_string(v, &m.severity).unwrap_or_default();
    let severity = if !sev_raw.is_empty() {
        if value_in_list(&sev_raw, &m.critical_values) {
            Severity::Disaster
        } else if value_in_list(&sev_raw, &m.warning_values) {
            Severity::Warning
        } else {
            Severity::parse(&sev_raw).unwrap_or(Severity::Warning)
        }
    } else {
        Severity::Warning
    };

    let mut labels: Labels = BTreeMap::new();
    labels.insert("alertname".into(), name);
    if !ip.is_empty() {
        // Keep ip / alertIp / instance in sync ? enrich lookups usually match on `ip`.
        labels.insert("ip".into(), ip.clone());
        labels.insert("alertIp".into(), ip.clone());
        labels.insert("instance".into(), ip);
    }
    if !sev_raw.is_empty() {
        labels.insert("severity".into(), sev_raw);
    }
    for (k, path) in &m.labels {
        if let Some(val) = path_as_string(v, path) {
            labels.insert(k.clone(), val);
        }
    }

    let mut annotations: Labels = BTreeMap::new();
    if !description.is_empty() {
        annotations.insert("description".into(), truncate(&description, 2000));
        annotations.insert("summary".into(), truncate(&description, 500));
    }

    Some(IngressAlert {
        status,
        fingerprint,
        labels,
        annotations,
        severity,
        value,
        starts_at: None,
        ends_at: None,
    })
}

fn parse_with_field_mapping(
    v: &serde_json::Value,
    m: &FieldMapping,
) -> Result<Vec<IngressAlert>, String> {
    let items: Vec<&serde_json::Value> = if !m.list.is_empty() {
        json_path_value(v, &m.list)
            .and_then(|x| x.as_array())
            .map(|arr| arr.iter().collect())
            .ok_or_else(|| format!("map_list `{}` is not an array", m.list))?
    } else if let Some(arr) = v.as_array() {
        arr.iter().collect()
    } else {
        vec![v]
    };

    let alerts: Vec<_> = items.into_iter().filter_map(|item| parse_mapped_alert(item, m)).collect();
    if alerts.is_empty() {
        Err("field mapping produced no alerts (check map_status fire/resolve values)".into())
    } else {
        Ok(alerts)
    }
}

/// Auto-detect Alertmanager / Generic / Probe payloads (raw bytes or log lines).
pub fn parse_ingress_payload(raw: &[u8]) -> Result<Vec<IngressAlert>, String> {
    parse_ingress_payload_with_options(raw, &BTreeMap::new())
}

/// Parse ingress payload; when `options` contain field mapping (`map_*`), use that first.
pub fn parse_ingress_payload_with_options(
    raw: &[u8],
    options: &BTreeMap<String, String>,
) -> Result<Vec<IngressAlert>, String> {
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let json_text = extract_json_payload(text);
    let mapping = FieldMapping::from_options(options);

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_text) {
        if mapping.enabled {
            return parse_with_field_mapping(&v, &mapping);
        }
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

    Err("payload is not alertmanager/generic/probe JSON (or configure map_* field mapping)".into())
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
        format!("{t}?")
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
            escalate_after_seconds: 0,
            escalate_severity: None,
            escalate_channel_ids: vec![],
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
            severity: Severity::Disaster,
            value: Some(99.0),
            starts_at: None,
            ends_at: None,
        };
        let (ev, t) = apply_ingress(&route(), &incoming, None, Utc::now());
        assert_eq!(ev.status, AlertStatus::Firing);
        assert_eq!(t, AlertTransition::BecameFiring);
        assert!(ev.fingerprint.contains("fp1"));
        assert_eq!(ev.tally, 1);
    }

    #[test]
    fn ingress_tally_bumps_and_resets() {
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
        let t0 = Utc::now();
        let (e1, _) = apply_ingress(&r, &firing, None, t0);
        assert_eq!(e1.tally, 1);
        let t1 = t0 + chrono::Duration::seconds(3);
        let (e2, tr) = apply_ingress(&r, &firing, Some(e1), t1);
        assert_eq!(tr, AlertTransition::Unchanged);
        assert_eq!(e2.tally, 2);
        assert_eq!(e2.last_occurrence_at, t1);

        let resolved = IngressAlert {
            status: AlertStatus::Resolved,
            fingerprint: Some("fp1".into()),
            labels: labels.clone(),
            annotations: BTreeMap::new(),
            severity: Severity::Warning,
            value: None,
            starts_at: None,
            ends_at: None,
        };
        let (er, _) = apply_ingress(&r, &resolved, Some(e2), t1);
        let t2 = t1 + chrono::Duration::seconds(2);
        let (again, tr2) = apply_ingress(&r, &firing, Some(er), t2);
        assert_eq!(tr2, AlertTransition::BecameFiring);
        assert_eq!(again.tally, 1);
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
            "retMessage":"????","retTimeMs":12976,"areacode":"jyq",
            "applicationName":"??????","bizchainName":"????-??????",
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
            Some("????-??????")
        );
        assert_eq!(alerts[0].value, Some(12976.0));
        assert_eq!(
            alerts[0].labels.get("alertIp").map(|s| s.as_str()),
            Some("127.0.0.1")
        );
        assert_eq!(
            alerts[0].labels.get("ip").map(|s| s.as_str()),
            Some("127.0.0.1")
        );

        let recover = r#"2026-07-21 17:00:29.175 [Worker] INFO  alertLogger - {"eventType":"recover","eventTime":"2026-07-21 17:00:28","messageId":"2077682656462446593","resultFlag":"GOOD","retCode":"4000","retMessage":"??","retTimeMs":26010,"bizchainName":"????-??????","alertCategory":"business"}"#;
        let alerts = parse_ingress_payload(recover.as_bytes()).unwrap();
        assert_eq!(alerts[0].status, AlertStatus::Resolved);
        assert_eq!(
            alerts[0].fingerprint.as_deref(),
            Some("2077682656462446593")
        );
    }

    #[test]
    fn parse_probe_infra_critical() {
        let raw = r#"{"eventType":"fire","resultFlag":"BAD","retCode":"10001","retMessage":"????","alertCategory":"infra","messageId":"m1","bizchainName":"?"}"#;
        let a = parse_probe_alert(&serde_json::from_str(raw).unwrap()).unwrap();
        assert_eq!(a.severity, Severity::Disaster);
    }

    #[test]
    fn parse_custom_field_mapping() {
        let raw = r#"{
            "data": {
                "items": [{
                    "state": "ALARM",
                    "title": "???",
                    "msg": "disk > 90%",
                    "host": "10.0.0.8",
                    "metric": 93.5,
                    "id": "evt-1",
                    "level": "P1"
                }]
            }
        }"#;
        let mut opts = BTreeMap::new();
        opts.insert("map_list".into(), "data.items".into());
        opts.insert("map_status".into(), "state".into());
        opts.insert("map_fire".into(), "ALARM,firing".into());
        opts.insert("map_resolve".into(), "OK,resolved".into());
        opts.insert("map_name".into(), "title".into());
        opts.insert("map_description".into(), "msg".into());
        opts.insert("map_ip".into(), "host".into());
        opts.insert("map_value".into(), "metric".into());
        opts.insert("map_fingerprint".into(), "id".into());
        opts.insert("map_severity".into(), "level".into());
        opts.insert("map_critical".into(), "P1,critical".into());

        let alerts = parse_ingress_payload_with_options(raw.as_bytes(), &opts).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].status, AlertStatus::Firing);
        assert_eq!(alerts[0].severity, Severity::Disaster);
        assert_eq!(alerts[0].labels.get("alertname").map(|s| s.as_str()), Some("???"));
        assert_eq!(alerts[0].labels.get("alertIp").map(|s| s.as_str()), Some("10.0.0.8"));
        assert_eq!(alerts[0].value, Some(93.5));
        assert_eq!(alerts[0].fingerprint.as_deref(), Some("evt-1"));
        assert!(alerts[0]
            .annotations
            .get("description")
            .unwrap()
            .contains("disk"));
    }

    #[test]
    fn parse_field_mapping_ip_before_underscore() {
        let raw = r#"{
            "status": 2,
            "summary": "disk high",
            "sourceciname": "82.12.161.32_kylin",
            "sourcealertkey": "vfs.dev.util[vdb]",
            "sourceidentifier": "82.12.161.32_kylin_vfs.dev.util[vdb]",
            "sourceseverity": "High"
        }"#;
        let mut opts = BTreeMap::new();
        opts.insert("map_status".into(), "status".into());
        opts.insert("map_fire".into(), "1,2".into());
        opts.insert("map_resolve".into(), "0".into());
        opts.insert("map_name".into(), "sourcealertkey".into());
        opts.insert("map_description".into(), "summary".into());
        opts.insert("map_ip".into(), "sourceciname|before:_".into());
        opts.insert("map_fingerprint".into(), "sourceidentifier".into());
        opts.insert("map_severity".into(), "sourceseverity".into());
        opts.insert("map_critical".into(), "High,Disaster,Critical".into());

        let alerts = parse_ingress_payload_with_options(raw.as_bytes(), &opts).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0].labels.get("alertIp").map(|s| s.as_str()),
            Some("82.12.161.32")
        );
        assert_eq!(
            alerts[0].labels.get("ip").map(|s| s.as_str()),
            Some("82.12.161.32")
        );
        assert_eq!(
            alerts[0].labels.get("instance").map(|s| s.as_str()),
            Some("82.12.161.32")
        );
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
        assert_eq!(alerts[0].severity, Severity::Disaster);
        assert!(alerts[0].ends_at.is_none());
    }
}
