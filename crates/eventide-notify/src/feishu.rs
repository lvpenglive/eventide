//! Feishu / Lark custom bot notifier.

use crate::{build_text, NotifyError};
use eventide_core::{AlertEvent, AlertTransition, NotifyChannel, Rule};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FeishuNotifier;

#[derive(Serialize)]
struct FsContent {
    text: String,
}

#[derive(Serialize)]
struct FsBody {
    msg_type: &'static str,
    content: FsContent,
}

impl FeishuNotifier {
    pub async fn send(
        http: &reqwest::Client,
        channel: &NotifyChannel,
        rule: &Rule,
        event: &AlertEvent,
        transition: AlertTransition,
    ) -> Result<(), NotifyError> {
        let url = signed_url(&channel.url, channel.secret.as_deref())?;
        let body = FsBody {
            msg_type: "text",
            content: FsContent {
                text: build_text(rule, event, transition),
            },
        };

        let resp = http.post(&url).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(NotifyError::Channel(format!(
                "feishu http {status}: {text}"
            )));
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            // Feishu returns code==0 on success
            if let Some(code) = v.get("code").and_then(|c| c.as_i64()) {
                if code != 0 {
                    return Err(NotifyError::Channel(format!("feishu err: {text}")));
                }
            }
        }
        Ok(())
    }
}

fn signed_url(base: &str, secret: Option<&str>) -> Result<String, NotifyError> {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else {
        return Ok(base.to_string());
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| NotifyError::Channel(e.to_string()))?
        .as_secs() as i64;

    // Feishu: string_to_sign = "{timestamp}\n{secret}", then HMAC-SHA256, base64
    let string_to_sign = format!("{timestamp}\n{secret}");
    let sign = hmac_sha256(secret.as_bytes(), string_to_sign.as_bytes());
    let sign_b64 = base64_encode(&sign);

    let sep = if base.contains('?') { "&" } else { "?" };
    Ok(format!(
        "{base}{sep}timestamp={timestamp}&sign={}",
        urlencoding_encode(&sign_b64)
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
