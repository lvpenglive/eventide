//! Alert enrichment: annotation templates, inline maps, and shared lookup tables.
//!
//! Applied after fingerprint is fixed and before silence / notify, so added
//! labels do not split alerts but can still participate in silence matching.

use crate::models::{AlertEvent, Labels, Severity};
use crate::template::{render_map, render_template, TemplateContext};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Enrichment strategy (informational; a rule may combine multiple mechanisms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnrichKind {
    /// Render annotation templates and merge into the event.
    AnnotationTemplate,
    /// Look up `labels[match_key]` in an inline mapping table.
    LabelMap,
    /// Look up against one or more shared [`LookupTable`]s via `lookup_table_ids`.
    Lookup,
    /// Two or more of template / label_map / lookup are configured together.
    Composite,
}

impl EnrichKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AnnotationTemplate => "annotation_template",
            Self::LabelMap => "label_map",
            Self::Lookup => "lookup",
            Self::Composite => "composite",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "annotation_template" | "template" | "annotations" => Some(Self::AnnotationTemplate),
            "label_map" | "map" | "mapping" => Some(Self::LabelMap),
            "lookup" | "lookup_table" | "external" => Some(Self::Lookup),
            "composite" | "combined" | "all" | "auto" | "" => Some(Self::Composite),
            _ => None,
        }
    }
}

impl EnrichRule {
    pub fn uses_lookup(&self) -> bool {
        !self.lookup_table_ids.is_empty()
    }

    pub fn uses_label_map(&self) -> bool {
        !self.match_key.trim().is_empty() && !self.mappings.is_empty()
    }

    pub fn uses_templates(&self) -> bool {
        !self.templates.is_empty()
    }

    pub fn uses_field_templates(&self) -> bool {
        !self.field_templates.is_empty()
    }

    pub fn uses_label_extracts(&self) -> bool {
        !self.label_extracts.is_empty()
    }

    /// Infer stored kind from which sections are filled.
    pub fn infer_kind(
        templates: &Labels,
        mappings: &BTreeMap<String, Labels>,
        lookup_ids: &[Uuid],
        match_key: &str,
        field_templates: &Labels,
        label_extracts: &Labels,
    ) -> EnrichKind {
        let has_tpl = !templates.is_empty()
            || !field_templates.is_empty()
            || !label_extracts.is_empty();
        let has_map = !match_key.trim().is_empty() && !mappings.is_empty();
        let has_lookup = !lookup_ids.is_empty();
        match (has_tpl, has_map, has_lookup) {
            (true, false, false) => EnrichKind::AnnotationTemplate,
            (false, true, false) => EnrichKind::LabelMap,
            (false, false, true) => EnrichKind::Lookup,
            (false, false, false) => EnrichKind::Composite,
            _ => EnrichKind::Composite,
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
    /// Suggested label key for this table (used by enrich lookup; each table keeps its own).
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
    /// For `LabelMap` only: which event label to look up in `mappings`.
    /// Lookup tables always use each table's own `key_label` (so one rule can
    /// attach host ledger by `ip` and severity ledger by `severity` together).
    #[serde(default)]
    pub match_key: String,
    /// For `AnnotationTemplate`: annotation key → template string.
    #[serde(default)]
    pub templates: Labels,
    /// For `LabelMap`: match value → key/value pairs to merge.
    #[serde(default)]
    pub mappings: BTreeMap<String, Labels>,
    /// For `Lookup`: shared table ids (many-to-many; applied in list order).
    #[serde(default)]
    pub lookup_table_ids: Vec<Uuid>,
    /// Per-table match label override for this rule: `table_id` → label name
    /// (e.g. use extracted `sss_ip` instead of the table's default `ip`).
    #[serde(default)]
    pub lookup_match_keys: BTreeMap<String, String>,
    /// Rewrite first-class alert fields after enrichment (templates).
    /// Supported keys: `severity`, `ip`, `alertname`, `summary`.
    #[serde(default)]
    pub field_templates: Labels,
    /// Run **before** lookup/map: write labels from templates (supports `|before:` etc.).
    /// Example: `ip` → `{{annotations.summary|before:_}}` so a ledger can match on `ip`.
    #[serde(default)]
    pub label_extracts: Labels,
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
        title: None,
        transition: None,
        labels_json: None,
        annotations_json: None,
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
        // Order: extract labels (for lookup keys) → lookup/map → templates.
        let mut applied = false;
        if rule.uses_label_extracts() {
            apply_label_extracts(event, rule_name, rule);
            applied = true;
        }
        if rule.uses_lookup() {
            apply_lookup(event, rule, lookups);
            applied = true;
        }
        if rule.uses_label_map() {
            apply_label_map(event, rule);
            applied = true;
        }
        if rule.uses_templates() {
            apply_annotation_template(event, rule_name, rule);
            applied = true;
        }
        if rule.uses_field_templates() {
            apply_field_templates(event, rule_name, rule);
            applied = true;
        }
        // Legacy fallback: kind set but content somehow empty / only kind-driven.
        if !applied {
            match rule.kind {
                EnrichKind::LabelMap => apply_label_map(event, rule),
                EnrichKind::Lookup => apply_lookup(event, rule, lookups),
                EnrichKind::AnnotationTemplate => {
                    apply_annotation_template(event, rule_name, rule)
                }
                EnrichKind::Composite => {}
            }
        }
    }

    // Final pass: expand templates in all annotations (covers rule-static ones).
    let rendered = {
        let ctx = template_ctx(event, rule_name);
        render_map(&event.annotations, &ctx)
    };
    event.annotations = rendered;
}

fn apply_label_extracts(event: &mut AlertEvent, rule_name: Option<&str>, rule: &EnrichRule) {
    if rule.label_extracts.is_empty() {
        return;
    }
    let rendered: Vec<(String, String)> = {
        let ctx = template_ctx(event, rule_name);
        rule.label_extracts
            .iter()
            .map(|(k, tmpl)| (k.clone(), render_template(tmpl, &ctx).trim().to_string()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect()
    };
    for (k, v) in rendered {
        event.labels.insert(k.clone(), v.clone());
        event.annotations.insert(k, v);
    }
}

fn merge_extra(event: &mut AlertEvent, extra: &Labels, write_labels: bool) {
    for (k, v) in extra {
        event.annotations.insert(k.clone(), v.clone());
        if write_labels {
            event.labels.insert(k.clone(), v.clone());
        }
    }
}

/// Sanitize lookup table name for namespaced label keys (`table.col`).
fn lookup_ns(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '\u{4e00}'..='\u{9fff}' => c,
            _ => '_',
        })
        .collect();
    if s.is_empty() {
        "lookup".into()
    } else {
        s
    }
}

fn merge_lookup_extra(
    event: &mut AlertEvent,
    table_name: &str,
    extra: &Labels,
    write_labels: bool,
) {
    let ns = lookup_ns(table_name);
    for (k, v) in extra {
        // Always namespace as `{table}.{col}` so templates are unambiguous across ledgers.
        let namespaced = format!("{ns}.{k}");
        event.annotations.insert(namespaced.clone(), v.clone());
        if write_labels {
            event.labels.insert(namespaced, v.clone());
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
    for tid in &rule.lookup_table_ids {
        let Some(table) = lookups.get(tid) else {
            continue;
        };
        if !table.enabled {
            continue;
        }
        // Per-rule override (e.g. extracted sss_ip) → else table default key_label.
        let tid_s = tid.to_string();
        let key = rule
            .lookup_match_keys
            .get(&tid_s)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| table.key_label.trim());
        if key.is_empty() {
            continue;
        }
        let Some(val) = event.labels.get(key).cloned() else {
            continue;
        };
        let Some(extra) = table.rows.get(&val) else {
            continue;
        };
        merge_lookup_extra(event, &table.name, extra, rule.write_labels);
    }
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

/// Apply templates that rewrite first-class alert fields (severity / ip / name / summary).
fn apply_field_templates(event: &mut AlertEvent, rule_name: Option<&str>, rule: &EnrichRule) {
    // Severity first so later templates (e.g. summary) see the updated value.
    if let Some(tmpl) = rule
        .field_templates
        .iter()
        .find(|(k, _)| matches!(k.as_str(), "severity" | "sev" | "level" | "级别"))
        .map(|(_, t)| t.clone())
    {
        let val = {
            let ctx = template_ctx(event, rule_name);
            render_template(&tmpl, &ctx).trim().to_string()
        };
        if let Some(sev) = parse_severity_loose(&val) {
            event.severity = sev;
            event.labels.insert("severity".into(), sev.as_str().into());
        }
    }

    let rendered: Vec<(String, String)> = {
        let ctx = template_ctx(event, rule_name);
        rule.field_templates
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "severity" | "sev" | "level" | "级别"))
            .map(|(k, tmpl)| (k.clone(), render_template(tmpl, &ctx).trim().to_string()))
            .filter(|(_, v)| !v.is_empty())
            .collect()
    };
    for (key, val) in rendered {
        match key.as_str() {
            "ip" | "alert_ip" | "alertIp" | "instance" => {
                event.labels.insert("ip".into(), val.clone());
                event.labels.insert("alertIp".into(), val.clone());
                if key == "instance" || !event.labels.contains_key("instance") {
                    event.labels.insert("instance".into(), val);
                }
            }
            "alertname" | "name" | "告警名称" => {
                event.labels.insert("alertname".into(), val);
            }
            "summary" | "description" | "告警描述" => {
                event.annotations.insert("summary".into(), val);
            }
            other => {
                event.annotations.insert(other.to_string(), val);
            }
        }
    }
}

fn parse_severity_loose(s: &str) -> Option<Severity> {
    if let Some(sev) = Severity::parse(s) {
        return Some(sev);
    }
    match s.trim() {
        "未分类" | "未知" => Some(Severity::NotClassified),
        "信息" | "提示" | "低" | "P5" => Some(Severity::Information),
        "警告" | "告警" | "P4" => Some(Severity::Warning),
        "一般严重" | "次要" | "中" | "P3" => Some(Severity::Average),
        "严重" | "重要" | "高" | "P2" => Some(Severity::High),
        "灾难" | "紧急" | "致命" | "P0" | "P1" => Some(Severity::Disaster),
        _ => None,
    }
}

/// Parse Omnibus / probe-style `.lookup` text.
///
/// Format (tab or multi-space separated):
/// ```text
/// #$ip	$cabinet	$brand	$usagedesc
/// 21.13.0.32	生产中心机房SC-T06	华为	电子渠道综合前置
/// ```
///
/// Header columns use `$name`; the key column may be marked with `#` (`#$ip`).
/// When no `#` is present, the first column is the lookup key.
/// Returns `(key_label, rows)` where `key_label` is the key column name without `$`/`#`.
pub fn parse_lookup_text(text: &str) -> Result<(String, BTreeMap<String, Labels>), String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with(';')
        })
        .collect();
    if lines.is_empty() {
        return Err("lookup 文件为空".into());
    }

    let header_fields = split_lookup_fields(lines[0]);
    if header_fields.is_empty() {
        return Err("lookup 表头无效".into());
    }

    let mut key_idx = 0usize;
    let mut found_key_mark = false;
    let mut names: Vec<String> = Vec::with_capacity(header_fields.len());
    for (i, raw) in header_fields.iter().enumerate() {
        let (is_key, name) = normalize_lookup_column(raw);
        if name.is_empty() {
            return Err(format!("第 {} 列表头无效: {raw}", i + 1));
        }
        if is_key {
            key_idx = i;
            found_key_mark = true;
        }
        names.push(name);
    }
    if !found_key_mark {
        key_idx = 0;
    }
    let key_label = names[key_idx].clone();

    let mut rows: BTreeMap<String, Labels> = BTreeMap::new();
    let ncols = names.len();
    for (line_no, line) in lines.iter().skip(1).enumerate() {
        let fields = split_lookup_row(line, ncols);
        if fields.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if fields.len() <= key_idx {
            return Err(format!(
                "第 {} 行字段不足（表头 {} 列，本行只解析到 {} 列）。字段请用 Tab 或空格分隔。",
                line_no + 2,
                ncols,
                fields.len()
            ));
        }
        let key = fields[key_idx].trim().to_string();
        if key.is_empty() {
            continue;
        }
        let mut attrs = Labels::new();
        for (i, name) in names.iter().enumerate() {
            if i == key_idx {
                continue;
            }
            let val = fields.get(i).map(|s| s.trim()).unwrap_or("");
            if !val.is_empty() {
                attrs.insert(name.clone(), val.to_string());
            }
        }
        rows.insert(key, attrs);
    }
    Ok((key_label, rows))
}

/// Render rows back to `.lookup` text (key column marked with `#`).
pub fn format_lookup_text(key_label: &str, rows: &BTreeMap<String, Labels>) -> String {
    let mut attr_keys: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for attrs in rows.values() {
        for k in attrs.keys() {
            if seen.insert(k.clone()) {
                attr_keys.push(k.clone());
            }
        }
    }
    attr_keys.sort();

    let mut out = String::new();
    out.push('#');
    out.push('$');
    out.push_str(key_label.trim_start_matches('$').trim_start_matches('#'));
    for k in &attr_keys {
        out.push('\t');
        out.push('$');
        out.push_str(k);
    }
    out.push('\n');

    for (key, attrs) in rows {
        out.push_str(key);
        for k in &attr_keys {
            out.push('\t');
            if let Some(v) = attrs.get(k) {
                out.push_str(v);
            }
        }
        out.push('\n');
    }
    out
}

fn split_lookup_fields(line: &str) -> Vec<String> {
    let line = line.trim_end_matches('\r').trim();
    if line.is_empty() {
        return Vec::new();
    }
    if line.contains('\t') {
        return line.split('\t').map(|s| s.trim().to_string()).collect();
    }
    // Space / whitespace separated (common when pasting from notepad without tabs).
    line.split_whitespace().map(|s| s.to_string()).collect()
}

/// Split a data row into `ncols` fields.
/// When there are more whitespace tokens than columns (values contain spaces),
/// keep the first column and trailing columns aligned; join overflow into the
/// last "middle" column (typical for free-text alias / description).
fn split_lookup_row(line: &str, ncols: usize) -> Vec<String> {
    let mut fields = split_lookup_fields(line);
    if ncols == 0 {
        return fields;
    }
    if fields.len() == ncols {
        return fields;
    }
    if line.contains('\t') || fields.len() < ncols {
        fields.resize(ncols, String::new());
        return fields;
    }
    // fields.len() > ncols, whitespace-separated with overflow.
    if ncols == 1 {
        return vec![fields.join(" ")];
    }
    if ncols == 2 {
        return vec![fields[0].clone(), fields[1..].join(" ")];
    }
    // Preserve last column; merge extras into the last middle column.
    let last = fields[fields.len() - 1].clone();
    let mid_cols = ncols - 2;
    let mid_tokens = &fields[1..fields.len() - 1];
    let mut out = Vec::with_capacity(ncols);
    out.push(fields[0].clone());
    if mid_tokens.len() <= mid_cols {
        out.extend(mid_tokens.iter().cloned());
        while out.len() < ncols - 1 {
            out.push(String::new());
        }
    } else {
        let keep = mid_cols - 1;
        out.extend(mid_tokens.iter().take(keep).cloned());
        out.push(mid_tokens[keep..].join(" "));
    }
    out.push(last);
    out
}

fn normalize_lookup_column(raw: &str) -> (bool, String) {
    let s = raw.trim();
    let is_key = s.starts_with('#');
    let s = s.trim_start_matches('#').trim();
    let s = s.trim_start_matches('$').trim();
    (is_key, s.to_string())
}

/// Auto-detect JSON object vs `.lookup` text and return rows (+ optional key from header).
pub fn parse_lookup_rows_auto(
    text: &str,
) -> Result<(Option<String>, BTreeMap<String, Labels>), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok((None, BTreeMap::new()));
    }
    if trimmed.starts_with('{') {
        let rows: BTreeMap<String, Labels> = serde_json::from_str(trimmed)
            .map_err(|e| format!("JSON 无效: {e}"))?;
        return Ok((None, rows));
    }
    let (key, rows) = parse_lookup_text(trimmed)?;
    Ok((Some(key), rows))
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
            severity: Severity::Disaster,
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
            lookup_table_ids: vec![],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
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
            lookup_table_ids: vec![table_id],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: false,
            enabled: true,
            priority: 5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert_eq!(
            ev.annotations.get("cmdb.owner").map(String::as_str),
            Some("alice")
        );
        assert_eq!(
            ev.annotations.get("cmdb.biz").map(String::as_str),
            Some("pay")
        );
        assert!(ev.labels.get("cmdb.owner").is_none());
        assert!(ev.labels.get("owner").is_none());
    }

    #[test]
    fn lookup_rule_can_use_multiple_tables() {
        let mut ev = sample_event();
        ev.labels.insert("service".into(), "api".into());

        let host_id = Uuid::new_v4();
        let svc_id = Uuid::new_v4();
        let mut host_rows = BTreeMap::new();
        let mut host_meta = BTreeMap::new();
        host_meta.insert("owner".into(), "alice".into());
        host_rows.insert("10.0.0.1".into(), host_meta);
        let mut svc_rows = BTreeMap::new();
        let mut svc_meta = BTreeMap::new();
        svc_meta.insert("tier".into(), "prod".into());
        svc_rows.insert("api".into(), svc_meta);

        let mut lookups = BTreeMap::new();
        lookups.insert(
            host_id,
            LookupTable {
                id: host_id,
                name: "hosts".into(),
                description: String::new(),
                key_label: "instance".into(),
                rows: host_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        lookups.insert(
            svc_id,
            LookupTable {
                id: svc_id,
                name: "services".into(),
                description: String::new(),
                key_label: "service".into(),
                rows: svc_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "multi-lookup".into(),
            kind: EnrichKind::Lookup,
            matchers: BTreeMap::new(),
            match_key: String::new(),
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_ids: vec![host_id, svc_id],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: false,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert_eq!(
            ev.annotations.get("hosts.owner").map(String::as_str),
            Some("alice")
        );
        assert_eq!(
            ev.annotations.get("services.tier").map(String::as_str),
            Some("prod")
        );
    }

    #[test]
    fn multi_lookup_ignores_rule_match_key() {
        // rule.match_key must not force every ledger to use the same label.
        let mut ev = sample_event();
        ev.labels.insert("severity".into(), "High".into());

        let host_id = Uuid::new_v4();
        let sev_id = Uuid::new_v4();
        let mut host_rows = BTreeMap::new();
        let mut host_meta = BTreeMap::new();
        host_meta.insert("主机名".into(), "node-a".into());
        host_rows.insert("10.0.0.1".into(), host_meta);

        let mut sev_rows = BTreeMap::new();
        let mut sev_meta = BTreeMap::new();
        sev_meta.insert("级别".into(), "disaster".into());
        sev_rows.insert("High".into(), sev_meta);

        let mut lookups = BTreeMap::new();
        lookups.insert(
            host_id,
            LookupTable {
                id: host_id,
                name: "hosts".into(),
                description: String::new(),
                key_label: "instance".into(),
                rows: host_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        lookups.insert(
            sev_id,
            LookupTable {
                id: sev_id,
                name: "levels".into(),
                description: String::new(),
                key_label: "severity".into(),
                rows: sev_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "multi".into(),
            kind: EnrichKind::Lookup,
            matchers: BTreeMap::new(),
            // Would previously break severity ledger by forcing key=ip on both.
            match_key: "ip".into(),
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_ids: vec![host_id, sev_id],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: true,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert!(ev.labels.get("主机名").is_none());
        assert!(ev.labels.get("级别").is_none());
        assert_eq!(
            ev.labels.get("hosts.主机名").map(String::as_str),
            Some("node-a")
        );
        assert_eq!(
            ev.labels.get("levels.级别").map(String::as_str),
            Some("disaster")
        );
    }

    #[test]
    fn lookup_column_collision_keeps_namespaced() {
        let mut ev = sample_event();
        let a_id = Uuid::new_v4();
        let b_id = Uuid::new_v4();
        let mut a_rows = BTreeMap::new();
        let mut a_meta = BTreeMap::new();
        a_meta.insert("owner".into(), "from-a".into());
        a_rows.insert("10.0.0.1".into(), a_meta);
        let mut b_rows = BTreeMap::new();
        let mut b_meta = BTreeMap::new();
        b_meta.insert("owner".into(), "from-b".into());
        b_rows.insert("10.0.0.1".into(), b_meta);
        let mut lookups = BTreeMap::new();
        lookups.insert(
            a_id,
            LookupTable {
                id: a_id,
                name: "table_a".into(),
                description: String::new(),
                key_label: "instance".into(),
                rows: a_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        lookups.insert(
            b_id,
            LookupTable {
                id: b_id,
                name: "table_b".into(),
                description: String::new(),
                key_label: "instance".into(),
                rows: b_rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "collide".into(),
            kind: EnrichKind::Lookup,
            matchers: BTreeMap::new(),
            match_key: String::new(),
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_ids: vec![a_id, b_id],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: true,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert!(ev.labels.get("owner").is_none());
        assert_eq!(
            ev.labels.get("table_a.owner").map(String::as_str),
            Some("from-a")
        );
        assert_eq!(
            ev.labels.get("table_b.owner").map(String::as_str),
            Some("from-b")
        );
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
            lookup_table_ids: vec![],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: false,
            enabled: true,
            priority: 50,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, Some("HighCPU"), &[rule], &empty_lookups());
        assert_eq!(
            ev.annotations.get("runbook").map(String::as_str),
            Some("https://wiki/api#disaster")
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
            lookup_table_ids: vec![],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: false,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &empty_lookups());
        assert!(ev.annotations.get("x").is_none());
    }

    #[test]
    fn parse_omnibus_lookup_text() {
        let text = "\
#$ip\t$cabinet\t$brand\t$usagedesc
21.13.0.32\t生产中心机房SC-T06\t华为\t电子渠道综合前置, 网上支付
172.18.27.51\t科技大厦2号机房KJ-2I05\t惠普\t大数据平台
";
        let (key, rows) = parse_lookup_text(text).unwrap();
        assert_eq!(key, "ip");
        assert_eq!(rows.len(), 2);
        let r = rows.get("21.13.0.32").unwrap();
        assert_eq!(r.get("cabinet").map(String::as_str), Some("生产中心机房SC-T06"));
        assert_eq!(r.get("brand").map(String::as_str), Some("华为"));
        assert_eq!(
            r.get("usagedesc").map(String::as_str),
            Some("电子渠道综合前置, 网上支付")
        );
    }

    #[test]
    fn parse_lookup_first_column_is_key_without_hash() {
        let text = "$host\t$owner\napi-1\talice\n";
        let (key, rows) = parse_lookup_text(text).unwrap();
        assert_eq!(key, "host");
        assert_eq!(rows.get("api-1").unwrap().get("owner").unwrap(), "alice");
    }

    #[test]
    fn format_lookup_roundtrip() {
        let text = "#$ip\t$brand\n1.1.1.1\t华为\n";
        let (key, rows) = parse_lookup_text(text).unwrap();
        let out = format_lookup_text(&key, &rows);
        let (key2, rows2) = parse_lookup_text(&out).unwrap();
        assert_eq!(key, key2);
        assert_eq!(rows, rows2);
    }

    #[test]
    fn parse_space_separated_without_dollar() {
        let text = "\
#ip 主机名 别名 联系人 电话
21.1.11.11 DX-AAM 业务拨测监控系统 张非 19989789999
";
        let (key, rows) = parse_lookup_text(text).unwrap();
        assert_eq!(key, "ip");
        let r = rows.get("21.1.11.11").unwrap();
        assert_eq!(r.get("主机名").map(String::as_str), Some("DX-AAM"));
        assert_eq!(r.get("别名").map(String::as_str), Some("业务拨测监控系统"));
        assert_eq!(r.get("联系人").map(String::as_str), Some("张非"));
        assert_eq!(r.get("电话").map(String::as_str), Some("19989789999"));
    }

    #[test]
    fn parse_space_separated_merges_middle_overflow() {
        let text = "\
#ip 主机名 别名 联系人
10.0.0.1 host1 业务 拨测 系统 张三
";
        let (key, rows) = parse_lookup_text(text).unwrap();
        assert_eq!(key, "ip");
        let r = rows.get("10.0.0.1").unwrap();
        assert_eq!(r.get("主机名").map(String::as_str), Some("host1"));
        assert_eq!(r.get("别名").map(String::as_str), Some("业务 拨测 系统"));
        assert_eq!(r.get("联系人").map(String::as_str), Some("张三"));
    }

    #[test]
    fn composite_rule_applies_lookup_map_and_template() {
        let mut ev = sample_event();
        ev.labels.insert("ip".into(), "10.0.0.1".into());

        let table_id = Uuid::new_v4();
        let mut rows = BTreeMap::new();
        let mut meta = BTreeMap::new();
        meta.insert("主机名".into(), "api-1".into());
        rows.insert("10.0.0.1".into(), meta);
        let mut lookups = BTreeMap::new();
        lookups.insert(
            table_id,
            LookupTable {
                id: table_id,
                name: "hosts".into(),
                description: String::new(),
                key_label: "ip".into(),
                rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        let mut mappings = BTreeMap::new();
        let mut map_meta = BTreeMap::new();
        map_meta.insert("tier".into(), "prod".into());
        mappings.insert("10.0.0.1".into(), map_meta);

        let mut templates = BTreeMap::new();
        templates.insert(
            "summary".into(),
            "{{labels.hosts.主机名}} / {{labels.tier}} value={{value}}".into(),
        );

        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "combo".into(),
            kind: EnrichKind::Composite,
            matchers: BTreeMap::new(),
            match_key: "ip".into(),
            templates,
            mappings,
            lookup_table_ids: vec![table_id],
            lookup_match_keys: BTreeMap::new(),
            field_templates: BTreeMap::new(),
            label_extracts: BTreeMap::new(),
            write_labels: true,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert_eq!(
            ev.labels.get("hosts.主机名").map(String::as_str),
            Some("api-1")
        );
        assert!(ev.labels.get("主机名").is_none());
        assert_eq!(ev.labels.get("tier").map(String::as_str), Some("prod"));
        assert_eq!(
            ev.annotations.get("summary").map(String::as_str),
            Some("api-1 / prod value=99")
        );
    }

    #[test]
    fn field_templates_set_severity_and_ip() {
        let mut ev = sample_event();
        let mut field_templates = BTreeMap::new();
        field_templates.insert("severity".into(), "警告".into());
        field_templates.insert("ip".into(), "{{labels.instance}}".into());
        field_templates.insert(
            "summary".into(),
            "主机 {{labels.instance}} 级别 {{severity}}".into(),
        );
        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "fields".into(),
            kind: EnrichKind::Composite,
            matchers: BTreeMap::new(),
            match_key: String::new(),
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_ids: vec![],
            lookup_match_keys: BTreeMap::new(),
            field_templates,
            label_extracts: BTreeMap::new(),
            write_labels: false,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &empty_lookups());
        assert_eq!(ev.severity, Severity::Warning);
        assert_eq!(ev.labels.get("ip").map(String::as_str), Some("10.0.0.1"));
        assert_eq!(ev.labels.get("alertIp").map(String::as_str), Some("10.0.0.1"));
        assert_eq!(
            ev.annotations.get("summary").map(String::as_str),
            Some("主机 10.0.0.1 级别 warning")
        );
    }

    #[test]
    fn label_extract_before_lookup_as_match_key() {
        let mut ev = sample_event();
        ev.annotations.insert(
            "summary".into(),
            "82.12.161.32_kylin disk high".into(),
        );

        let table_id = Uuid::new_v4();
        let mut rows = BTreeMap::new();
        let mut meta = BTreeMap::new();
        meta.insert("主机名".into(), "kylin-node".into());
        rows.insert("82.12.161.32".into(), meta);
        let mut lookups = BTreeMap::new();
        lookups.insert(
            table_id,
            LookupTable {
                id: table_id,
                name: "hosts".into(),
                description: String::new(),
                key_label: "ip".into(),
                rows,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        let mut label_extracts = BTreeMap::new();
        label_extracts.insert(
            "sss_ip".into(),
            "{{annotations.summary|before:_}}".into(),
        );
        let mut field_templates = BTreeMap::new();
        field_templates.insert(
            "summary".into(),
            "{{labels.sss_ip}} · {{labels.hosts.主机名}}".into(),
        );
        let mut lookup_match_keys = BTreeMap::new();
        lookup_match_keys.insert(table_id.to_string(), "sss_ip".into());

        let rule = EnrichRule {
            id: Uuid::new_v4(),
            name: "extract+lookup".into(),
            kind: EnrichKind::Composite,
            matchers: BTreeMap::new(),
            match_key: String::new(),
            templates: BTreeMap::new(),
            mappings: BTreeMap::new(),
            lookup_table_ids: vec![table_id],
            lookup_match_keys,
            field_templates,
            label_extracts,
            write_labels: true,
            enabled: true,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        enrich_alert(&mut ev, None, &[rule], &lookups);
        assert_eq!(
            ev.labels.get("sss_ip").map(String::as_str),
            Some("82.12.161.32")
        );
        assert_eq!(
            ev.labels.get("hosts.主机名").map(String::as_str),
            Some("kylin-node")
        );
        assert_eq!(
            ev.annotations.get("summary").map(String::as_str),
            Some("82.12.161.32 · kylin-node")
        );
    }
}
