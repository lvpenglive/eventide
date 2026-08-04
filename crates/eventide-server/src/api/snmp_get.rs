//! SNMPv2c GET for MIB browser probe (debug / OID verification).

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use snmp2::{Oid, SyncSession, Value as SnmpValue};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/snmp/get", post(snmp_get))
}

#[derive(Debug, Deserialize)]
struct SnmpGetBody {
    host: String,
    #[serde(default)]
    oid: String,
    #[serde(default = "default_community")]
    community: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

fn default_community() -> String {
    "public".into()
}
fn default_port() -> u16 {
    161
}
fn default_timeout_ms() -> u64 {
    3000
}

fn err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "ok": false, "error": msg.into() })))
}

fn parse_oid_nums(s: &str) -> Result<Vec<u64>, String> {
    let t = s.trim().trim_start_matches('.');
    if t.is_empty() {
        return Err("oid is required".into());
    }
    let mut out = Vec::new();
    for part in t.split('.') {
        let n: u64 = part
            .parse()
            .map_err(|_| format!("invalid oid component: {part}"))?;
        out.push(n);
    }
    if out.is_empty() {
        return Err("oid is required".into());
    }
    Ok(out)
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn format_snmp_value(v: &SnmpValue<'_>) -> (String, String) {
    match v {
        SnmpValue::Boolean(b) => ("Boolean".into(), b.to_string()),
        SnmpValue::Integer(i) => ("Integer".into(), i.to_string()),
        SnmpValue::OctetString(b) => {
            let s = String::from_utf8_lossy(b);
            if s.chars().all(|c| !c.is_control() || c == '\t' || c == '\n') {
                ("OctetString".into(), s.into_owned())
            } else {
                ("OctetString".into(), format!("0x{}", to_hex(b)))
            }
        }
        SnmpValue::Null => ("Null".into(), "null".into()),
        SnmpValue::ObjectIdentifier(oid) => ("OID".into(), format!("{oid}")),
        SnmpValue::IpAddress(ip) => (
            "IpAddress".into(),
            format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]),
        ),
        SnmpValue::Counter32(n) => ("Counter32".into(), n.to_string()),
        SnmpValue::Unsigned32(n) => ("Unsigned32".into(), n.to_string()),
        SnmpValue::Timeticks(n) => ("Timeticks".into(), n.to_string()),
        SnmpValue::Opaque(b) => ("Opaque".into(), format!("0x{}", to_hex(b))),
        SnmpValue::Counter64(n) => ("Counter64".into(), n.to_string()),
        other => ("Unknown".into(), format!("{other:?}")),
    }
}

async fn snmp_get(
    State(_st): State<Arc<AppState>>,
    Json(body): Json<SnmpGetBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let host = body.host.trim();
    if host.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "host is required"));
    }
    if host.len() > 255 {
        return Err(err(StatusCode::BAD_REQUEST, "host too long"));
    }
    let oid_str = body.oid.trim();
    let nums = parse_oid_nums(oid_str).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let oid = Oid::from(&nums).map_err(|e| err(StatusCode::BAD_REQUEST, format!("invalid oid: {e:?}")))?;

    let port = if body.port == 0 { 161 } else { body.port };
    let community = if body.community.trim().is_empty() {
        "public".to_string()
    } else {
        body.community.trim().to_string()
    };
    let timeout_ms = body.timeout_ms.clamp(500, 30_000);
    let addr = format!("{host}:{port}");

    // Resolve once so SyncSession gets a concrete socket addr string.
    let resolved = tokio::task::spawn_blocking({
        let addr = addr.clone();
        move || addr.to_socket_addrs().map(|mut i| i.next())
    })
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| err(StatusCode::BAD_REQUEST, format!("resolve host: {e}")))?
    .ok_or_else(|| err(StatusCode::BAD_REQUEST, "host resolve returned no address"))?;

    let target = resolved.to_string();
    let community_bytes = community.into_bytes();
    let timeout = Duration::from_millis(timeout_ms);

    let result = tokio::time::timeout(
        timeout + Duration::from_millis(500),
        tokio::task::spawn_blocking(move || -> Result<(String, String, String), String> {
            let mut sess = SyncSession::new_v2c(&target, &community_bytes, Some(timeout), 0)
                .map_err(|e| format!("snmp session: {e}"))?;
            let response = sess.get(&oid).map_err(|e| format!("snmp get: {e}"))?;
            if response.error_status != snmp2::snmp::ERRSTATUS_NOERROR {
                return Err(format!(
                    "snmp error_status={} error_index={}",
                    response.error_status, response.error_index
                ));
            }
            let mut it = response.varbinds;
            let Some((got_oid, val)) = it.next() else {
                return Err("empty varbind response".into());
            };
            let (ty, value) = format_snmp_value(&val);
            Ok((got_oid.to_string(), ty, value))
        }),
    )
    .await
    .map_err(|_| err(StatusCode::GATEWAY_TIMEOUT, format!("timeout after {timeout_ms}ms")))?
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;

    let (got_oid, ty, value) = result;
    Ok(Json(json!({
        "ok": true,
        "host": host,
        "port": port,
        "oid": got_oid,
        "request_oid": oid_str,
        "type": ty,
        "value": value,
    })))
}
