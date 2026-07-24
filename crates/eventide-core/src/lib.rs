//! Core types for Eventide: rules, alert events, channels, and evaluation helpers.

mod enrich;
mod eval;
mod fingerprint;
mod ingress;
mod models;
mod template;

pub use enrich::{
    enrich_alert, format_lookup_text, matchers_match, parse_lookup_rows_auto, parse_lookup_text,
    EnrichKind, EnrichRule, LookupTable,
};
pub use eval::{
    alert_title, apply_evaluation, compare, evaluate_rule, evaluate_sample, merge_labels,
    EvaluateResult, MetricSample,
};
pub use fingerprint::alert_fingerprint;
pub use ingress::{
    apply_ingress, extract_json_payload, ingress_alert_name, looks_like_probe_alert,
    parse_alertmanager, parse_generic, parse_ingress_payload, parse_ingress_payload_with_options,
    parse_mapped_alert, parse_probe_alert, FieldMapping, AlertmanagerWebhook, GenericAlert,
    GenericWebhook,
};
pub use models::*;
pub use template::{
    apply_string_transform, render_map, render_template, split_path_transform, TemplateContext,
};