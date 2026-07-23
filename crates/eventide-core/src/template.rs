//! Simple `{{...}}` template rendering for alert annotations.

use crate::models::Labels;
use std::fmt::Write as _;

/// Context available while expanding annotation templates.
#[derive(Debug, Clone, Copy)]
pub struct TemplateContext<'a> {
    pub labels: &'a Labels,
    pub annotations: &'a Labels,
    pub value: Option<f64>,
    pub severity: &'a str,
    pub status: &'a str,
    pub fingerprint: &'a str,
    pub rule_name: Option<&'a str>,
}

/// Expand `{{labels.x}}` / `{{annotations.x}}` / `{{value}}` / `{{severity}}` /
/// `{{status}}` / `{{fingerprint}}` / `{{rule.name}}` placeholders.
///
/// Unknown placeholders expand to an empty string. Nested braces are not supported.
pub fn render_template(input: &str, ctx: &TemplateContext<'_>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        match rest.find("}}") {
            Some(end) => {
                let key = rest[..end].trim();
                out.push_str(&resolve(key, ctx));
                rest = &rest[end + 2..];
            }
            None => {
                out.push_str("{{");
                out.push_str(rest);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Render every value in a label/annotation map.
pub fn render_map(map: &Labels, ctx: &TemplateContext<'_>) -> Labels {
    map.iter()
        .map(|(k, v)| (k.clone(), render_template(v, ctx)))
        .collect()
}

fn resolve(key: &str, ctx: &TemplateContext<'_>) -> String {
    if let Some(rest) = key.strip_prefix("labels.") {
        return ctx.labels.get(rest).cloned().unwrap_or_default();
    }
    if let Some(rest) = key.strip_prefix("annotations.") {
        return ctx.annotations.get(rest).cloned().unwrap_or_default();
    }
    match key {
        "value" => ctx
            .value
            .map(|v| {
                let mut s = String::new();
                let _ = write!(s, "{v}");
                s
            })
            .unwrap_or_default(),
        "severity" => ctx.severity.to_string(),
        "status" => ctx.status.to_string(),
        "fingerprint" => ctx.fingerprint.to_string(),
        "rule.name" | "rule" => ctx.rule_name.unwrap_or("").to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn ctx<'a>(
        labels: &'a Labels,
        annotations: &'a Labels,
    ) -> TemplateContext<'a> {
        TemplateContext {
            labels,
            annotations,
            value: Some(95.5),
            severity: "critical",
            status: "firing",
            fingerprint: "abc",
            rule_name: Some("HighCPU"),
        }
    }

    #[test]
    fn renders_labels_and_builtins() {
        let mut labels = BTreeMap::new();
        labels.insert("instance".into(), "db-1".into());
        let annotations = BTreeMap::new();
        let c = ctx(&labels, &annotations);
        assert_eq!(
            render_template(
                "host={{labels.instance}} val={{value}} sev={{severity}} rule={{rule.name}}",
                &c
            ),
            "host=db-1 val=95.5 sev=critical rule=HighCPU"
        );
    }

    #[test]
    fn unknown_is_empty() {
        let labels = BTreeMap::new();
        let annotations = BTreeMap::new();
        let c = ctx(&labels, &annotations);
        assert_eq!(render_template("x{{labels.missing}}y", &c), "xy");
    }

    #[test]
    fn unclosed_left_intact() {
        let labels = BTreeMap::new();
        let annotations = BTreeMap::new();
        let c = ctx(&labels, &annotations);
        assert_eq!(render_template("hello {{world", &c), "hello {{world");
    }
}
