//! Shared helpers for channel message envelopes.

use eventide_core::NotifyChannel;

/// `text` (default) or `markdown`.
pub(crate) fn msg_type(channel: &NotifyChannel) -> &str {
    channel
        .options
        .get("msg_type")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("text")
}

pub(crate) fn is_markdown(channel: &NotifyChannel) -> bool {
    matches!(
        msg_type(channel).to_ascii_lowercase().as_str(),
        "markdown" | "md"
    )
}

pub(crate) fn at_all(channel: &NotifyChannel) -> bool {
    matches!(
        channel
            .options
            .get("at_all")
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

pub(crate) fn at_mobiles(channel: &NotifyChannel) -> Vec<String> {
    channel
        .options
        .get("at_mobiles")
        .map(|s| {
            s.split(|c| c == ',' || c == ';' || c == ' ' || c == '\n')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn option<'a>(channel: &'a NotifyChannel, key: &str) -> Option<&'a str> {
    channel
        .options
        .get(key)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// First non-empty line, truncated — used as DingTalk markdown title.
pub(crate) fn first_line_title(content: &str) -> String {
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("Eventide");
    let stripped: String = line
        .chars()
        .filter(|c| *c != '#' && *c != '*' && *c != '`')
        .collect::<String>()
        .trim()
        .to_string();
    let title = if stripped.is_empty() {
        "Eventide"
    } else {
        stripped.as_str()
    };
    title.chars().take(64).collect()
}
