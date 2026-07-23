//! Core types for Eventide: rules, alert events, channels, and evaluation helpers.

mod eval;
mod fingerprint;
mod ingress;
mod models;

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
