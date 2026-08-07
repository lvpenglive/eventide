//! Mutation audit middleware — records who changed what via the console API.

use crate::auth::Claims;
use crate::state::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use eventide_core::AuditLog;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;

/// After auth: log successful/failed mutating API calls (skip probes / tests).
pub async fn audit_mutators(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_uppercase();
    let write = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    let path = req.uri().path().to_string();
    let claims = req.extensions().get::<Claims>().cloned();
    let client_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip().to_string());

    if !write || !should_audit(&path) {
        return next.run(req).await;
    }

    let (action, resource_type, resource_id) = classify(&method, &path);
    let response = next.run(req).await;
    let status_code = response.status().as_u16() as i32;

    if let Some(claims) = claims {
        let actor = claims.sub.trim();
        if !actor.is_empty() {
            let actor_uid = Uuid::parse_str(claims.uid.trim()).ok();
            let log = AuditLog {
                id: Uuid::new_v4(),
                created_at: Utc::now(),
                actor_username: actor.to_string(),
                actor_uid,
                action,
                resource_type,
                resource_id,
                method,
                path,
                status_code,
                detail_json: None,
                client_ip,
            };
            if let Err(e) = state.db.insert_audit_log(&log) {
                tracing::warn!(error = %e, "audit log insert failed");
            }
        }
    }

    response
}

fn should_audit(path: &str) -> bool {
    if path.starts_with("/trap-api") {
        return false;
    }
    if path.starts_with("/api/ingress/kafka/") {
        return false;
    }
    if path.ends_with("/test") || path.ends_with("/evaluate") || path.ends_with("/preview") {
        return false;
    }
    if path.starts_with("/api/snmp") {
        return false;
    }
    // Keep password change; skip nothing else special here.
    true
}

fn classify(method: &str, path: &str) -> (String, String, Option<String>) {
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    // Expect ["api", resource, ...]
    if segs.first() != Some(&"api") || segs.len() < 2 {
        return (
            format!("{}.{}", "unknown", verb(method)),
            "unknown".into(),
            None,
        );
    }

    let resource = segs[1];
    let (resource_type, id_idx) = match resource {
        "alerts" => ("alert", 2usize),
        "silences" => ("silence", 2),
        "maintenance-windows" => ("maintenance", 2),
        "rules" => ("rule", 2),
        "channels" => ("channel", 2),
        "datasources" => ("datasource", 2),
        "enrich" => ("enrich", 2),
        "lookups" => ("lookup", 2),
        "ingress" => ("ingress", 2),
        "users" => ("user", 2),
        "roles" => ("role", 2),
        "departments" => ("department", 2),
        "settings" => ("settings", 2),
        "license" => ("license", 2),
        "mibs" => ("mib", 2),
        "policies" => ("policy", 2),
        "auth" => ("auth", 2),
        other => (other, 2),
    };

    let resource_id = segs.get(id_idx).copied().filter(|s| looks_like_id(s));

    let action = if resource == "alerts" && segs.len() >= 4 {
        match segs[3] {
            "ack" => "alert.ack".into(),
            "unack" => "alert.unack".into(),
            "close" => "alert.close".into(),
            other => format!("alert.{other}"),
        }
    } else if resource == "users" && segs.get(3) == Some(&"reset-password") {
        "user.reset_password".into()
    } else if resource == "auth" && segs.get(2) == Some(&"change-password") {
        "auth.change_password".into()
    } else if resource == "license" {
        format!("license.{}", verb(method))
    } else if resource == "settings" {
        let key = segs.get(2).unwrap_or(&"settings");
        format!("settings.{}.{}", key, verb(method))
    } else {
        format!("{}.{}", resource_type, verb(method))
    };

    (action, resource_type.into(), resource_id.map(|s| s.to_string()))
}

fn verb(method: &str) -> &'static str {
    match method {
        "POST" => "create",
        "PUT" | "PATCH" => "update",
        "DELETE" => "delete",
        _ => "write",
    }
}

fn looks_like_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    // UUID or opaque token-ish path segment
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !matches!(
            s,
            "ack"
                | "unack"
                | "close"
                | "test"
                | "evaluate"
                | "preview"
                | "reset-password"
                | "change-password"
                | "request"
                | "alert-history"
                | "trap-token"
                | "storm"
                | "kafka"
                | "notifies"
        )
}
