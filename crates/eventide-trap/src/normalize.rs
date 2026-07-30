//! Build Eventide Kafka Ingress compatible JSON (generic webhook shape).

use crate::config::TrapConfig;
use crate::parse::ParsedTrap;
use crate::policy_store::{
    build_fingerprint, build_template_vars, render_template, resolve_event_status,
    resolve_object_name, resolve_severity, TrapPolicy,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// One alert object matching Eventide generic / auto ingress parsing.
#[allow(dead_code)]
pub fn to_ingress_alert(trap: &ParsedTrap, cfg: &TrapConfig) -> Value {
    to_ingress_alert_with_policy(trap, cfg, None)
}

pub fn to_ingress_alert_with_policy(
    trap: &ParsedTrap,
    cfg: &TrapConfig,
    policy: Option<&TrapPolicy>,
) -> Value {
    let (alertname, severity, summary, policy_id, policy_module) = if let Some(p) = policy {
        let vars = build_template_vars(trap, p);
        let summary = if p.summary_template.trim().is_empty() {
            format!("SNMP Trap {} from {} ({})", trap.trap_oid, trap.peer_ip, p.name)
        } else {
            render_template(&p.summary_template, &vars)
        };
        (
            p.name.clone(),
            resolve_severity(p, trap),
            summary,
            Some(p.id.clone()),
            Some(p.module.clone()),
        )
    } else {
        let severity = trap
            .severity_hint
            .clone()
            .unwrap_or_else(|| cfg.default_severity.clone());
        let summary = format!(
            "SNMP Trap {} from {} ({})",
            trap.trap_oid, trap.peer_ip, trap.alertname
        );
        (trap.alertname.clone(), severity, summary, None, None)
    };

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    labels.insert("alertname".into(), alertname);
    labels.insert("ip".into(), trap.peer_ip.clone());
    labels.insert("trap_oid".into(), trap.trap_oid.clone());
    labels.insert("source".into(), "ingress:snmptrap".into());
    labels.insert("snmp_version".into(), trap.version.clone());
    if !trap.community.is_empty() {
        labels.insert("community".into(), trap.community.clone());
    }
    if let Some(mid) = policy_id {
        labels.insert("trap_policy_id".into(), mid);
    }
    if let Some(m) = policy_module.filter(|s| !s.is_empty()) {
        labels.insert("mib_module".into(), m);
    }

    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
    annotations.insert("summary".into(), summary);
    annotations.insert("snmp_note".into(), trap.raw_note.clone());
    if let Some(p) = policy {
        if !p.description.is_empty() {
            annotations.insert("description".into(), p.description.clone());
        }
        annotations.insert("policy_match".into(), p.match_mode.clone());
    }
    annotations.insert(
        "varbinds".into(),
        serde_json::to_string(&trap.varbinds).unwrap_or_else(|_| "{}".into()),
    );
    annotations.insert(
        "varbind_count".into(),
        trap.varbinds.len().to_string(),
    );
    let mut varbind_names = BTreeMap::new();
    if let Some(p) = policy {
        for oid in trap.varbinds.keys() {
            if let Some(name) = resolve_object_name(oid, &p.object_oids) {
                varbind_names.insert(oid.clone(), name);
            }
        }
    }
    if !varbind_names.is_empty() {
        annotations.insert(
            "varbind_names".into(),
            serde_json::to_string(&varbind_names).unwrap_or_else(|_| "{}".into()),
        );
    }
    for (i, (oid, val)) in trap.varbinds.iter().enumerate() {
        annotations.insert(format!("vb{i}_oid"), oid.clone());
        annotations.insert(format!("vb{i}_val"), truncate_val(val));
        if let Some(name) = varbind_names.get(oid) {
            annotations.insert(format!("vb{i}_name"), name.clone());
        }
    }

    let fingerprint = build_fingerprint(trap, policy);

    let status = match policy {
        Some(p) => resolve_event_status(p, trap),
        None => "firing",
    };
    let now = Utc::now().to_rfc3339();
    let mut alert = json!({
        "status": status,
        "fingerprint": fingerprint,
        "severity": severity,
        "labels": labels,
        "annotations": annotations,
        "startsAt": now,
    });
    if status == "resolved" {
        alert["endsAt"] = json!(now);
    }
    alert
}

fn truncate_val(val: &str) -> String {
    const MAX: usize = 4096;
    if val.len() <= MAX {
        val.to_string()
    } else {
        format!("{}…(+{}B)", &val[..MAX], val.len() - MAX)
    }
}

#[allow(dead_code)]
pub fn to_kafka_payload(trap: &ParsedTrap, cfg: &TrapConfig) -> Vec<u8> {
    to_kafka_payload_with_policy(trap, cfg, None)
}

pub fn to_kafka_payload_with_policy(
    trap: &ParsedTrap,
    cfg: &TrapConfig,
    policy: Option<&TrapPolicy>,
) -> Vec<u8> {
    let alert = to_ingress_alert_with_policy(trap, cfg, policy);
    serde_json::to_vec(&alert).unwrap_or_else(|_| b"{}".to_vec())
}

pub fn simulate_parsed(
    ip: &str,
    trap_oid: &str,
    alertname: Option<String>,
    severity: Option<String>,
    varbinds: BTreeMap<String, String>,
) -> ParsedTrap {
    let alertname = alertname.unwrap_or_else(|| {
        trap_oid
            .rsplit('.')
            .next()
            .map(|s| format!("snmpTrap-{s}"))
            .unwrap_or_else(|| "snmpTrap".into())
    });
    ParsedTrap {
        version: "simulate".into(),
        community: "public".into(),
        peer_ip: ip.to_string(),
        peer_port: 0,
        trap_oid: trap_oid.to_string(),
        alertname,
        severity_hint: severity,
        varbinds,
        raw_note: "http simulate".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TrapConfig;
    use crate::policy_store::TrapPolicy;

    #[test]
    fn ingress_shape() {
        let cfg = TrapConfig::default();
        let trap = simulate_parsed(
            "10.0.0.1",
            "1.3.6.1.6.3.1.1.5.3",
            Some("linkDown".into()),
            Some("disaster".into()),
            BTreeMap::new(),
        );
        let v = to_ingress_alert(&trap, &cfg);
        assert_eq!(v["status"], "firing");
        assert_eq!(v["labels"]["source"], "ingress:snmptrap");
        assert_eq!(v["labels"]["ip"], "10.0.0.1");
        assert!(v["fingerprint"].as_str().unwrap().contains("10.0.0.1"));
    }

    #[test]
    fn policy_overrides_name_and_summary() {
        let cfg = TrapConfig::default();
        let trap = simulate_parsed(
            "10.0.0.1",
            "1.3.6.1.6.3.1.1.5.3",
            Some("snmpTrap-3".into()),
            None,
            BTreeMap::new(),
        );
        let policy = TrapPolicy {
            id: "p1".into(),
            name: "linkDown".into(),
            trap_oid: "1.3.6.1.6.3.1.1.5.3".into(),
            match_mode: "exact".into(),
            severity: "disaster".into(),
            enabled: true,
            summary_template: "${alertname} on ${ip}".into(),
            description: "".into(),
            objects: vec![],
            object_oids: BTreeMap::new(),
            keywords: vec![],
            module: "IF-MIB".into(),
            status: "".into(),
            resolve_oid: "".into(),
            resolve_values: vec![],
            fingerprint_oids: vec![],
            severity_oid: "".into(),
            severity_map: BTreeMap::new(),
            updated_at: "".into(),
        };
        let v = to_ingress_alert_with_policy(&trap, &cfg, Some(&policy));
        assert_eq!(v["labels"]["alertname"], "linkDown");
        assert_eq!(v["severity"], "disaster");
        assert_eq!(v["status"], "firing");
        assert_eq!(v["annotations"]["summary"], "linkDown on 10.0.0.1");
        assert_eq!(v["labels"]["trap_policy_id"], "p1");
    }

    #[test]
    fn policy_resolve_by_varbind_value() {
        let cfg = TrapConfig::default();
        let mut vbs = BTreeMap::new();
        vbs.insert("1.3.6.1.2.1.2.2.1.1.5".into(), "5".into());
        vbs.insert("1.3.6.1.4.1.9.9.41.1.2.3.0".into(), "cleared".into());
        let trap = simulate_parsed(
            "10.0.0.1",
            "1.3.6.1.4.1.9.9.41.2.0.1",
            None,
            None,
            vbs,
        );
        let policy = TrapPolicy {
            id: "p2".into(),
            name: "ciscoAlarm".into(),
            trap_oid: "1.3.6.1.4.1.9.9.41.2.0.1".into(),
            match_mode: "exact".into(),
            severity: "warning".into(),
            enabled: true,
            summary_template: "alarm on ${ip}".into(),
            description: "".into(),
            objects: vec![],
            object_oids: BTreeMap::new(),
            keywords: vec![],
            module: "".into(),
            status: "".into(),
            resolve_oid: "1.3.6.1.4.1.9.9.41.1.2.3".into(),
            resolve_values: vec!["cleared".into()],
            fingerprint_oids: vec![],
            severity_oid: "".into(),
            severity_map: BTreeMap::new(),
            updated_at: "".into(),
        };
        let v = to_ingress_alert_with_policy(&trap, &cfg, Some(&policy));
        assert_eq!(v["status"], "resolved");
        assert!(v.get("endsAt").is_some());
    }

    #[test]
    fn all_varbinds_emitted() {
        let cfg = TrapConfig::default();
        let mut vbs = BTreeMap::new();
        for i in 0..20 {
            vbs.insert(format!("1.3.6.1.4.1.9.{i}"), format!("v{i}"));
        }
        let trap = simulate_parsed(
            "10.0.0.1",
            "1.3.6.1.4.1.25506.4.2.26.2.0.175",
            None,
            None,
            vbs,
        );
        let v = to_ingress_alert(&trap, &cfg);
        let an = v["annotations"].as_object().unwrap();
        assert_eq!(an["varbind_count"], "20");
        assert!(an.contains_key("vb19_oid"));
    }

    #[test]
    fn varbind_names_from_policy_object_oids() {
        let cfg = TrapConfig::default();
        let mut vbs = BTreeMap::new();
        vbs.insert(
            "1.3.6.1.4.1.2011.2.91.10.3.1.1.9.0".into(),
            "SN-123".into(),
        );
        vbs.insert("1.3.6.1.6.3.1.1.4.1.0".into(), "1.2.3".into());
        let trap = simulate_parsed(
            "10.0.0.1",
            "1.3.6.1.4.1.2011.2.91.10.2.1.0.1",
            None,
            None,
            vbs,
        );
        let mut object_oids = BTreeMap::new();
        object_oids.insert(
            "hwIsmReportingAlarmSerialNo".into(),
            "1.3.6.1.4.1.2011.2.91.10.3.1.1.9".into(),
        );
        let policy = TrapPolicy {
            id: "hw1".into(),
            name: "hwAlarm".into(),
            trap_oid: "1.3.6.1.4.1.2011.2.91.10.2.1.0.1".into(),
            match_mode: "exact".into(),
            severity: "warning".into(),
            enabled: true,
            summary_template: "".into(),
            description: "".into(),
            objects: vec![],
            object_oids,
            keywords: vec![],
            module: "ISM-HUAWEI-MIB".into(),
            status: "".into(),
            resolve_oid: "".into(),
            resolve_values: vec![],
            fingerprint_oids: vec![],
            severity_oid: "".into(),
            severity_map: BTreeMap::new(),
            updated_at: "".into(),
        };
        let v = to_ingress_alert_with_policy(&trap, &cfg, Some(&policy));
        let an = &v["annotations"];
        let names: BTreeMap<String, String> =
            serde_json::from_str(an["varbind_names"].as_str().unwrap()).unwrap();
        assert_eq!(
            names.get("1.3.6.1.4.1.2011.2.91.10.3.1.1.9.0").map(|s| s.as_str()),
            Some("hwIsmReportingAlarmSerialNo")
        );
        assert!(!names.contains_key("1.3.6.1.6.3.1.1.4.1.0"));
    }
}
