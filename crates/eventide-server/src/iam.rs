//! RBAC permission catalog and IAM models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct PermDef {
    pub code: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

pub const PERMISSION_CATALOG: &[PermDef] = &[
    PermDef {
        code: "overview:read",
        label: "查看总览",
        group: "运营",
    },
    PermDef {
        code: "alerts:read",
        label: "查看告警事件",
        group: "运营",
    },
    PermDef {
        code: "alerts:write",
        label: "管理告警事件",
        group: "运营",
    },
    PermDef {
        code: "silences:read",
        label: "查看静默策略",
        group: "运营",
    },
    PermDef {
        code: "silences:write",
        label: "管理静默策略",
        group: "运营",
    },
    PermDef {
        code: "datasources:read",
        label: "查看数据源",
        group: "接入",
    },
    PermDef {
        code: "datasources:write",
        label: "管理数据源",
        group: "接入",
    },
    PermDef {
        code: "rules:read",
        label: "查看告警规则",
        group: "接入",
    },
    PermDef {
        code: "rules:write",
        label: "管理告警规则",
        group: "接入",
    },
    PermDef {
        code: "ingress:read",
        label: "查看告警接入",
        group: "接入",
    },
    PermDef {
        code: "ingress:write",
        label: "管理告警接入",
        group: "接入",
    },
    PermDef {
        code: "channels:read",
        label: "查看通知渠道",
        group: "通知",
    },
    PermDef {
        code: "channels:write",
        label: "管理通知渠道",
        group: "通知",
    },
    PermDef {
        code: "enrich:read",
        label: "查看告警丰富",
        group: "通知",
    },
    PermDef {
        code: "enrich:write",
        label: "管理告警丰富",
        group: "通知",
    },
    PermDef {
        code: "users:read",
        label: "查看用户",
        group: "系统",
    },
    PermDef {
        code: "users:write",
        label: "管理用户",
        group: "系统",
    },
    PermDef {
        code: "roles:read",
        label: "查看角色权限",
        group: "系统",
    },
    PermDef {
        code: "roles:write",
        label: "管理角色权限",
        group: "系统",
    },
    PermDef {
        code: "departments:read",
        label: "查看部门",
        group: "系统",
    },
    PermDef {
        code: "departments:write",
        label: "管理部门",
        group: "系统",
    },
    PermDef {
        code: "settings:read",
        label: "查看系统设置",
        group: "系统",
    },
    PermDef {
        code: "settings:write",
        label: "修改系统设置",
        group: "系统",
    },
    PermDef {
        code: "trap:read",
        label: "查看 SNMP Trap",
        group: "接入",
    },
    PermDef {
        code: "trap:write",
        label: "管理 SNMP Trap / 试推送",
        group: "接入",
    },
];

pub fn catalog_json() -> serde_json::Value {
    serde_json::json!(PERMISSION_CATALOG
        .iter()
        .map(|p| {
            serde_json::json!({
                "code": p.code,
                "label": p.label,
                "group": p.group,
            })
        })
        .collect::<Vec<_>>())
}

pub fn has_perm(perms: &[String], need: &str) -> bool {
    perms.iter().any(|p| p == "*" || p == need)
}

/// Resolve required permission for a protected API path + method.
/// Returns None when only authentication is required (e.g. /api/auth/me).
pub fn route_permission(method: &str, path: &str) -> Option<&'static str> {
    let m = method.to_uppercase();
    let write = matches!(m.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");

    if path == "/api/auth/me" {
        return None;
    }
    if path == "/api/permissions" {
        return Some("roles:read");
    }
    if path.starts_with("/api/departments") {
        return Some(if write {
            "departments:write"
        } else {
            "departments:read"
        });
    }
    if path.starts_with("/api/roles") {
        return Some(if write { "roles:write" } else { "roles:read" });
    }
    if path.starts_with("/api/users") {
        return Some(if write { "users:write" } else { "users:read" });
    }
    if path == "/api/overview" {
        return Some("overview:read");
    }
    if path.starts_with("/api/alerts") {
        return Some(if write { "alerts:write" } else { "alerts:read" });
    }
    if path.starts_with("/api/silences") {
        return Some(if write {
            "silences:write"
        } else {
            "silences:read"
        });
    }
    if path.starts_with("/api/datasources") {
        return Some(if write {
            "datasources:write"
        } else {
            "datasources:read"
        });
    }
    if path.starts_with("/api/rules") {
        return Some(if write { "rules:write" } else { "rules:read" });
    }
    if path.starts_with("/api/ingress") {
        return Some(if write {
            "ingress:write"
        } else {
            "ingress:read"
        });
    }
    if path.starts_with("/api/channels") {
        return Some(if write {
            "channels:write"
        } else {
            "channels:read"
        });
    }
    if path.starts_with("/api/notifies") {
        return Some("channels:read");
    }
    if path.starts_with("/api/enrich") || path.starts_with("/api/lookups") {
        return Some(if write { "enrich:write" } else { "enrich:read" });
    }
    if path.starts_with("/api/settings")
        || path == "/api/license"
        || path.starts_with("/api/license/")
    {
        return Some(if write {
            "settings:write"
        } else {
            "settings:read"
        });
    }
    if path.starts_with("/api/mibs")
        || path.starts_with("/api/policies")
        || path.starts_with("/api/trap")
        || path.starts_with("/api/snmp")
        || path.starts_with("/trap-api")
    {
        return Some(if write { "trap:write" } else { "trap:read" });
    }
    // Unknown protected route: require login only
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Department {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub sort_order: i64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub department_id: Option<Uuid>,
    pub role_ids: Vec<Uuid>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserAccount {
    pub fn public_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "username": self.username,
            "display_name": self.display_name,
            "department_id": self.department_id,
            "role_ids": self.role_ids,
            "enabled": self.enabled,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
        })
    }
}
