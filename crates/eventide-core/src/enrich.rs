//! Alert enrichment: annotation templates, inline maps, and shared lookup tables.
//!
//! Applied after fingerprint is fixed and before silence / notify, so added
//! labels do not split alerts but can still participate in silence matching.

use crate::models::{AlertEvent, Labels};
use crate::template::{render_map, render_template, TemplateContext};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Enrichment strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichKind {
    /// Render annotation templates and merge into the event.
    AnnotationTemplate,
    /// Look up `labels[match_key]` in an inline mapping table.
    LabelMap,
    /// Look up against a shared [`LookupTable`] referenced by `lookup_table_id`.
    Lookup,
}

impl EnrichKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnnotationTemplate => "annotation_template",
            Self::LabelMap => "label_map",
            Self::Lookup => "lookup",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "annotation_template" | "template" | "annotations" => Some(Self::AnnotationTemplate),
            "label_map" | "map" | "mapping" => Some(Self::LabelMap),
            "lookup" | "lookup_table" | "external" => Some(Self::Lookup),
            _ => None,
        }
    }
}

/// Shared lookup table (外表) — reusable key → attribute rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupTable {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Suggested / default label key used when an enrich rule leaves `match_key` empty.
    #[serde(default = "default_key_label")]
    pub key_label: String,
    /// Lookup key → attributes to merge into the alert.
    #[serde(default)]
    pub rows: BTreeMap<String, Labels>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_key_label() -> String {
    "instance".into()
}

/// One enrichment rule stored in SQLite / exposed via API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichRule {
    pub id: Uuid,
    pub name: String,
    pub kind: EnrichKind,
    /// Only apply when all of these labels match the event.
    #[serde(default)]
    pub matchers: Labels,
    /// For `LabelMap` / `Lookup`: which event label to look up.
    /// For `Lookup`, falls back to the table's `key_label` when empty.
    #[serde(default)]
    pub match_key: String,
    /// For `AnnotationTemplate`: annotation key → template string.
    #[serde(default)]
    pub templates: Labels,
    /// For `LabelMap`: match value → key/value pairs to merge.
    #[serde(default)]
    pub mappings: BTreeMap<String, Labels>,
    /// For `Lookup`: shared table id.
    #[serde(default)]
    pub lookup_table_id: Option<Uuid>,
    /// When true, mapped fields are also written into `labels`
    /// (fingerprint is already fixed, so this is safe for grouping).
    #[serde(default)]
    pub write_labels: bool,
    pub enabled: bool,
    /// Lower runs first.
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Returns true when every matcher key equals the corresponding event label.
pub fn matchers_match(matchers: &Labels, labels: &Labels) -> bool {
    for (k, v) in matchers {
        match labels.get(k) {
            Some(lv) if lv == v => {}
            _ => return false,
        }
    }
    true
}

fn template_ctx<'a>(
    event: &'a AlertEvent,
    rule_name: Option<&'a str>,
) -> TemplateContext<'a> {
    TemplateContext {
        labels: &event.labels,
        annotations: &event.annotations,
        value: event.value,
        severity: event.severity.as_str(),
        status: event.status.as_str(),
        fingerprint: &event.fingerprint,
        rule_name,
    }
}

/// Apply enabled enrich rules (by priority), then expand remaining `{{...}}`
/// placeholders in annotations. Mutates `event` in place.
pub fn enrich_alert(
    event: &mut AlertEvent,
    rule_name: Option<&str>,
    rules: &[EnrichRule],
    lookups: &BTreeMap<Uuid, LookupTable>,
) {
    let mut ordered: Vec<&EnrichRule> = rules.iter().filter(|r| r.enabled).collect();
    ordered.sort_by_key(|r| (r.priority, r.name.as_str()));

    for rule in ordered {
        if !matchers_match(&rule.matchers, &event.labels) {
            continue;
        }
        match rule.kind {
            EnrichKind::LabelMap => apply_label_map(event, rule),
            EnrichKind::Lookup => apply_lookup(event, rule, lookups),
            EnrichKind::AnnotationTemplate => apply_annotation_template(event, rule_name, rule),
        }
    }

    // Final pass: expand templates in all annotations (covers rule-static ones).
    let rendered = {
        let ctx = template_ctx(event, rule_name);
        render_map(&event.annotations, &ctx)
    };
    event.annotations = rendered;
}

fn merge_extra(event: &mut AlertEvent, extra: &Labels, write_labels: bool) {
    for (k, v) in extra {
        event.annotations.insert(k.clone(), v.clone());
        if write_labels {
            event.labels.insert(k.clone(), v.clone());
        }
    }
}

fn apply_label_map(event: &mut AlertEvent, rule: &EnrichRule) {
    let key = rule.match_key.trim();
    if key.is_empty() {
        return;
    }
    let Some(val) = event.labels.get(key).cloned() else {
        return;
    };
    let Some(extra) = rule.mappings.get(&val) else {
        return;
    };
    merge_extra(event, extra, rule.write_labels);
}

fn apply_lookup(
    event: &mut AlertEvent,
    rule: &EnrichRule,
    lookups: &BTreeMap<Uuid, LookupTable>,
) {
    let Some(tid) = rule.lookup_table_id else {
        return;
    };
    let Some(table) = lookups.get(&tid) else {
        return;
    };
    if !table.enabled {
        return;
    }
    let key = if rule.match_key.trim().is_empty() {
        table.key_label.trim()
    } else {
        rule.match_key.trim()
    };
    if key.is_empty() {
        return;
    }
    let Some(val) = event.labels.get(key).cloned() else {
        return;
    };
    let Some(extra) = table.rows.get(&val) else {
        return;
    };
    merge_extra(event, extra, rule.write_labels);
}

fn apply_annotation_template(event: &mut AlertEvent, rule_name: Option<&str>, rule: &EnrichRule) {
    let rendered: Vec<(String, String)> = {
        let ctx = template_ctx(event, rule_name);
        rule.templates
            .iter()
            .map(|(k, tmpl)| (k.clone(), render_template(tmpl, &ctx)))
            .collect()
    };
    for (k, v) in rendered {
        event.annotations.insert(k, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AlertStatus, Severity};
    use chrono::Utc;

    fn sample_event() -> AlertEvent {
        let mut labels = BTreeMap::new();
        labels.insert("instance".into(), "10.0.0.1".into());
        labels.insert("job".into(), "api".into());
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "summary".into(),
            "CPU high on {{labels.instance}}".into(),
        );
        AlertEvent {
            id: Uuid::nil(),
            rule_id: Uuid::nil(),
            fingerprint: "fp1".into(),
            status: AlertStatus::Firing,
            severity: Severity::Critical,
            labels,
            annotations,
            value: Some(99.0),
            starts_at: Utc::now(),
            ends_at: None,
            pending_since: None,
            last_evaluated_at: Utc::now(),
            notified_firing: false,
            notified_resolved: false,
        }
    }

    fn empty_lookups() -> BTreeMap<Uuid, LookupTable> {
        BTreeMap::new()
    }

    #[test]
    fn expands_static_annotation_templates() {
        let mut ev = sample_event();
        enrich_alert(&mut ev, Some("HighCPU"), &[], &empty_lookups());
        assert_eq!(
            ev.annotations.get("summary").map(String::as_str),
            Some("CPU high on 10.0.0.1")
        );
    }

    #[test]
    fn label_map_enriches_annotations() {
        let mut ev = sample_event();
        let mut mappings = BTreeMap::new();
        let mut meta = BTreeMap::new();
        meta.insert("owner".into(), "alice".into());
        meta.insert("team".into(), "sre".into());
        mappings.insert("10.0.0.1".into(), meta);
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "cmdb".into(),
            kind: EnrichKind::LabelMap,
            matchers: BTreeMap::new(),
            match_key: "instance".into(),
            templates: BTreeMap::new(),
            mappings,
            lookup_table_id: None,
            write_labels: true,
            enabled: true,
            priority: 10,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &empty_lookups());
        assert_eq!(ev.annotations.get("owner").map(String::as_str), Some("alice"));
        assert_eq!(ev.labels.get("team").map(String::as_str), Some("sre"));
        assert_eq!(ev.fingerprint, "fp1");
    }

    #[test]
    fn lookup_table_enriches_from_shared_rows() {
        let mut ev = sample_event();
        let table_id = Uuid::new_v4();
        let mut rows = BTreeMap::new();
        let mut meta = BTreeMap::new();
        meta.insert("owner".into(), "alice".into());
        meta.insert("biz".into(), "pay".into());
        rows.insert("10.0.0.1".into(), meta);
        let mut lookups = BTreeMap::new();
        lookups.insert(
            table_id,
            LookupTable {
                id: table_id,
                name: "cmdb".into(),
                description: "hosts".into(),
                key_label: "instance".into(),
                rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "use-cmdb".into(),
            kind: EnrichKind::Lookup,
            matchers: BTreeMap::new(),
            match_key: String::new(), // fall back to table.key_label
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_id: Some(table_id),
            write_labels: false,
            enabled: true,
            priority: 5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert_eq!(ev.annotations.get("owner").map(String::as_str), Some("alice"));
        assert_eq!(ev.annotations.get("biz").map(String::as_str), Some("pay"));
        assert!(ev.labels.get("owner").is_none());
    }

    #[test]
    fn annotation_template_rule() {
        let mut ev = sample_event();
        let mut templates = BTreeMap::new();
        templates.insert(
            "runbook".into(),
            "https://wiki/{{labels.job}}#{{severity}}".into(),
        );
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "runbook".into(),
            kind: EnrichKind::AnnotationTemplate,
            matchers: BTreeMap::new(),
            match_key: String::new(),
            templates,
            mappings: BTreeMap::new(),
            lookup_table_id: None,
            write_labels: false,
            enabled: true,
            priority: 50,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, Some("HighCPU"), &[rule], &empty_lookups());
        assert_eq!(
            ev.annotations.get("runbook").map(String::as_str),
            Some("https://wiki/api#critical")
        );
    }

    #[test]
    fn matchers_gate_application() {
        let mut ev = sample_event();
        let mut matchers = BTreeMap::new();
        matchers.insert("job".into(), "db".into());
        let mut templates = BTreeMap::new();
        templates.insert("x".into(), "should-not-appear".into());
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "gated".into(),
            kind: EnrichKind::AnnotationTemplate,
            matchers,
            match_key: String::new(),
            templates,
            mappings: BTreeMap::new(),
            lookup_table_id: None,
            write_labels: false,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &empty_lookups());
        assert!(ev.annotations.get("x").is_none());
    }
}
