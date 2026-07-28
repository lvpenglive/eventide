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
    /// Optional preformatted title (notify templates).
    pub title: Option<&'a str>,
    /// Notify edge label: firing / resolved / unchanged.
    pub transition: Option<&'a str>,
    /// JSON dump of all labels.
    pub labels_json: Option<&'a str>,
    /// JSON dump of all annotations.
    pub annotations_json: Option<&'a str>,
}

/// Expand `{{labels.x}}` / `{{annotations.x}}` / `{{value}}` / `{{severity}}` /
/// `{{status}}` / `{{fingerprint}}` / `{{rule.name}}` / `{{title}}` /
/// `{{transition}}` / `{{labels}}` / `{{annotations}}` placeholders.
///
/// Path transforms (same as ingress mapping):
/// `{{annotations.summary|before:：}}`, `|after:`, `|split:SEP:INDEX`, `|between:START:END`.
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

/// Split `field|before:_` into (`field`, Some(`before:_`)).
pub fn split_path_transform(path: &str) -> (String, Option<String>) {
    let path = path.trim();
    if let Some((left, right)) = path.split_once('|') {
        let left = left.trim();
        // Keep spaces after `before:` / `after:` (e.g. `|before: `).
        let right = right.trim_start();
        if !left.is_empty() && !right.is_empty() {
            return (left.to_string(), Some(right.to_string()));
        }
    }
    (path.to_string(), None)
}

/// String transforms for templates / ingress paths.
///
/// - `before:SEP` — substring before first SEP
/// - `after:SEP` — substring after first SEP
/// - `split:SEP:INDEX` — split by SEP, take INDEX (0-based)
/// - `between:START:END` — from first START to next END（起点/终点可为空，空=串首/串尾）
/// - `json` — JSON string literal（含引号与转义，便于嵌入自定义 JSON 模板）
pub fn apply_string_transform(raw: &str, transform: Option<&str>) -> String {
    let Some(t) = transform.map(|s| s.trim_start()).filter(|s| !s.is_empty()) else {
        return raw.to_string();
    };
    if t == "json" {
        return serde_json::to_string(raw).unwrap_or_else(|_| "\"\"".into());
    }
    if let Some(sep) = t.strip_prefix("before:") {
        return raw
            .split_once(sep)
            .map(|(a, _)| a.to_string())
            .unwrap_or_else(|| raw.to_string());
    }
    if let Some(sep) = t.strip_prefix("after:") {
        return raw
            .split_once(sep)
            .map(|(_, b)| b.to_string())
            .unwrap_or_else(|| raw.to_string());
    }
    if let Some(rest) = t.strip_prefix("between:") {
        // between:START:END — split on first ':' after the prefix
        let (start, end) = rest.split_once(':').unwrap_or((rest, ""));
        let after_start = if start.is_empty() {
            raw
        } else if let Some((_, rest)) = raw.split_once(start) {
            rest
        } else {
            return String::new();
        };
        if end.is_empty() {
            return after_start.to_string();
        }
        return after_start
            .split_once(end)
            .map(|(mid, _)| mid.to_string())
            .unwrap_or_else(|| after_start.to_string());
    }
    if let Some(rest) = t.strip_prefix("split:") {
        if let Some((sep, idx_s)) = rest.rsplit_once(':') {
            if !sep.is_empty() {
                if let Ok(idx) = idx_s.parse::<usize>() {
                    let parts: Vec<&str> = raw.split(sep).collect();
                    if let Some(p) = parts.get(idx) {
                        return (*p).to_string();
                    }
                }
            }
        }
    }
    raw.to_string()
}

fn resolve(key: &str, ctx: &TemplateContext<'_>) -> String {
    let (path, transform) = split_path_transform(key);
    let raw = if let Some(rest) = path.strip_prefix("labels.") {
        ctx.labels.get(rest).cloned().unwrap_or_default()
    } else if let Some(rest) = path.strip_prefix("annotations.") {
        ctx.annotations.get(rest).cloned().unwrap_or_default()
    } else {
        match path.as_str() {
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
            "title" => ctx.title.unwrap_or("").to_string(),
            "transition" => ctx.transition.unwrap_or("").to_string(),
            "labels" => ctx.labels_json.unwrap_or("").to_string(),
            "annotations" => ctx.annotations_json.unwrap_or("").to_string(),
            _ => String::new(),
        }
    };
    apply_string_transform(&raw, transform.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn ctx<'a>(labels: &'a Labels, annotations: &'a Labels) -> TemplateContext<'a> {
        TemplateContext {
            labels,
            annotations,
            value: Some(95.5),
            severity: "critical",
            status: "firing",
            fingerprint: "abc",
            rule_name: Some("HighCPU"),
            title: None,
            transition: None,
            labels_json: None,
            annotations_json: None,
        }
    }

    #[test]
    fn renders_labels_and_builtins() {
        let mut labels = BTreeMap::new();
        labels.insert("instance".into(), "db-1".into());
        let annotations = BTreeMap::new();
        let c = ctx(&labels, &annotations);
        assert_eq!(
            render_template("host={{labels.instance}} sev={{severity}}", &c),
            "host=db-1 sev=critical"
        );
    }

    #[test]
    fn renders_path_transforms() {
        let labels = BTreeMap::new();
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "summary".into(),
            "麒麟主机当前系统磁盘[vdb] IO使用百分比为：97.49 %".into(),
        );
        annotations.insert("host".into(), "82.12.161.32_kylin".into());
        let c = ctx(&labels, &annotations);
        assert_eq!(
            render_template("{{annotations.host|before:_}}", &c),
            "82.12.161.32"
        );
        assert_eq!(
            render_template("{{annotations.host|after:_}}", &c),
            "kylin"
        );
        assert_eq!(
            render_template("{{annotations.summary|before:：}}", &c),
            "麒麟主机当前系统磁盘[vdb] IO使用百分比为"
        );
        assert_eq!(
            render_template("{{annotations.host|split:_:0}}", &c),
            "82.12.161.32"
        );
        assert_eq!(
            render_template("{{annotations.summary|between:为：: %}}", &c),
            "97.49"
        );
        assert_eq!(
            render_template("{{annotations.host|between:_:}}", &c),
            "kylin"
        );
        assert_eq!(
            render_template("{{annotations.host|between::.}}", &c),
            "82"
        );
    }
}
