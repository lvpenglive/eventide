//! DingTalk custom robot notifier (text / markdown + @).

use crate::channel_opts::{at_all, at_mobiles, first_line_title, is_markdown};
use crate::{build_text_for_channel, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DingTalkNotifier;

impl DingTalkNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let content = build_text_for_channel(channel, rule, event, transition);
        let report = Self::send_text_report(http, channel, &content).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report.error.unwrap_or_else(|| "dingtalk failed".into()),
            ))
        }
    }

    pub async fn send_text(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> Result<(), NotifyError> {
        let report = Self::send_text_report(http, channel, content).await;
        if report.ok {
            Ok(())
        } else {
            Err(NotifyError::Channel(
                report.error.unwrap_or_else(|| "dingtalk failed".into()),
            ))
        }
    }

    pub async fn send_text_report(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        content: &str,
    ) -> crate::NotifySendReport {
        let url = match signed_url(&channel.url, channel.secret.as_deref()) {
            Ok(u) => u,
            Err(e) => {
                return crate::NotifySendReport {
                    ok: false,
                    kind: "dingtalk".into(),
                    request_url: channel.url.clone(),
                    request_body: build_body(channel, content),
                    http_status: None,
                    response_body: String::new(),
                    error: Some(e.to_string()),
                };
            }
        };
        let body = build_body(channel, content);
        crate::post_json_report(http, "dingtalk", url, body, |t| {
            crate::errcode_nonzero(t, "errcode", "dingtalk")
        })
        .await
    }
}

fn build_body(channel: &NotifyChannel, content: &str) -> serde_json::Value {
    let mobiles = at_mobiles(channel);
    let at = serde_json::json!({
        "atMobiles": mobiles,
        "isAtAll": at_all(channel),
    });
    if is_markdown(channel) {
        serde_json::json!({
            "msgtype": "markdown",
            "markdown": {
                "title": first_line_title(content),
                "text": content,
            },
            "at": at,
        })
    } else {
        serde_json::json!({
            "msgtype": "text",
            "text": { "content": content },
            "at": at,
        })
    }
}

fn signed_url(base: &str, secret: Option<&str>) -> Result<String, NotifyError> {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else {
        return Ok(base.to_string());
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| NotifyError::Channel(e.to_string()))?
        .as_millis() as i64;

    let string_to_sign = format!("{timestamp}\n{secret}");
    let sign = hmac_sha256(secret.as_bytes(), string_to_sign.as_bytes());
    let sign_b64 = base64_encode(&sign);
    let sign_enc = urlencoding_encode(&sign_b64);

    let sep = if base.contains('?') { "&" } else { "?" };
    Ok(format!(
        "{base}{sep}timestamp={timestamp}&sign={sign_enc}"
    ))
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = Sha256::digest(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let result = outer.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let a = chunk[0] as u32;
        let b = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let c = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (a << 16) | (b << 8) | c;
        out.push(T[((triple >> 18) & 0x3f) as usize] as char);
        out.push(T[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
