//! Persist and match Trap policies (from MIB export / CRUD).

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::mib_store::{PolicyExport, TrapPolicyDraft};
use crate::parse::ParsedTrap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapPolicy {
    pub id: String,
    pub name: String,
    pub trap_oid: String,
    /// `exact` | `prefix`
    #[serde(default = "default_match")]
    pub match_mode: String,
    pub severity: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub summary_template: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub objects: Vec<String>,
    /// Symbolic object name → numeric OID (from MIB export).
    #[serde(default)]
    pub object_oids: BTreeMap<String, String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub module: String,
    /// Legacy: when `resolved` and no resolve_oid/values, this trap OID is always a recovery event
    /// (e.g. linkUp). Prefer `resolve_oid` + `resolve_values` for same-OID fire/clear.
    #[serde(default)]
    pub status: String,
    /// Varbind OID or OBJECTS name whose value decides recovery.
    #[serde(default)]
    pub resolve_oid: String,
    /// If that varbind's value equals any of these (case-insensitive) → `resolved`; else `firing`.
    #[serde(default)]
    pub resolve_values: Vec<String>,
    /// Varbind OID(s) / OBJECTS names whose **values** form the alert fingerprint
    /// (e.g. Huawei alarm serial `…1.9`). Empty → fallback: peer + trap_oid + varbind OID list.
    #[serde(default)]
    pub fingerprint_oids: Vec<String>,
    /// Varbind OID / OBJECTS name for dynamic severity (e.g. Huawei fault level `…1.6`).
    #[serde(default)]
    pub severity_oid: String,
    /// Raw varbind value → Eventide severity (`1=disaster;2=high`). Miss → policy.severity.
    #[serde(default)]
    pub severity_map: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: String,
}

fn default_match() -> String {
    "exact".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PolicyFile {
    version: u32,
    policies: Vec<TrapPolicy>,
}

pub struct PolicyStore {
    pool: mysql::Pool,
    /// Display label for API (`mysql:trap_policies`).
    backend: String,
    inner: RwLock<Vec<TrapPolicy>>,
    /// Last seen MAX(updated_at) for hot reload.
    stamp: RwLock<String>,
}

impl PolicyStore {
    pub async fn open(pool: mysql::Pool, migrate_from: Option<&Path>) -> Result<Self> {
        crate::db::ensure_trap_schema(&pool)?;
        let store = Self {
            pool,
            backend: "mysql:trap_policies".into(),
            inner: RwLock::new(Vec::new()),
            stamp: RwLock::new(String::new()),
        };
        store.reload_from_db().await?;
        if store.inner.read().await.is_empty() {
            if let Some(path) = migrate_from {
                if path.exists() {
                    store.migrate_from_json(path).await?;
                }
            }
        }
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        Path::new(self.backend.as_str())
    }

    pub fn backend(&self) -> &str {
        &self.backend
    }

    fn load_all_sync(pool: &mysql::Pool) -> Result<Vec<TrapPolicy>> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let mut result = conn
            .query_iter(
                r#"SELECT id, name, trap_oid, match_mode, severity, enabled,
                          summary_template, description, objects_json, object_oids_json,
                          keywords_json, module, status, resolve_oid, resolve_values_json,
                          fingerprint_oids_json, severity_oid, severity_map_json, updated_at
                   FROM trap_policies ORDER BY trap_oid, match_mode"#,
            )
            .context("select trap_policies")?;
        let mut out = Vec::new();
        for row in result.by_ref() {
            let row = row.context("policy row")?;
            out.push(policy_from_row(row)?);
        }
        Ok(out)
    }

    fn max_updated_sync(pool: &mysql::Pool) -> Result<String> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let v: Option<Option<String>> =
            conn.exec_first("SELECT MAX(updated_at) FROM trap_policies", ())?;
        Ok(v.flatten().unwrap_or_default())
    }

    fn upsert_db_sync(pool: &mysql::Pool, p: &TrapPolicy) -> Result<()> {
        use mysql::params;
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let objects_json = serde_json::to_string(&p.objects)?;
        let object_oids_json = serde_json::to_string(&p.object_oids)?;
        let keywords_json = serde_json::to_string(&p.keywords)?;
        let resolve_values_json = serde_json::to_string(&p.resolve_values)?;
        let fingerprint_oids_json = serde_json::to_string(&p.fingerprint_oids)?;
        let severity_map_json = serde_json::to_string(&p.severity_map)?;
        conn.exec_drop(
            r#"INSERT INTO trap_policies (
                id, name, trap_oid, match_mode, severity, enabled,
                summary_template, description, objects_json, object_oids_json,
                keywords_json, module, status, resolve_oid, resolve_values_json,
                fingerprint_oids_json, severity_oid, severity_map_json, updated_at
            ) VALUES (
                :id, :name, :trap_oid, :match_mode, :severity, :enabled,
                :summary_template, :description, :objects_json, :object_oids_json,
                :keywords_json, :module, :status, :resolve_oid, :resolve_values_json,
                :fingerprint_oids_json, :severity_oid, :severity_map_json, :updated_at
            ) ON DUPLICATE KEY UPDATE
                name=VALUES(name), trap_oid=VALUES(trap_oid), match_mode=VALUES(match_mode),
                severity=VALUES(severity), enabled=VALUES(enabled),
                summary_template=VALUES(summary_template), description=VALUES(description),
                objects_json=VALUES(objects_json), object_oids_json=VALUES(object_oids_json),
                keywords_json=VALUES(keywords_json), module=VALUES(module), status=VALUES(status),
                resolve_oid=VALUES(resolve_oid), resolve_values_json=VALUES(resolve_values_json),
                fingerprint_oids_json=VALUES(fingerprint_oids_json),
                severity_oid=VALUES(severity_oid), severity_map_json=VALUES(severity_map_json),
                updated_at=VALUES(updated_at)"#,
            params! {
                "id" => &p.id,
                "name" => &p.name,
                "trap_oid" => &p.trap_oid,
                "match_mode" => &p.match_mode,
                "severity" => &p.severity,
                "enabled" => if p.enabled { 1i8 } else { 0i8 },
                "summary_template" => &p.summary_template,
                "description" => &p.description,
                "objects_json" => objects_json,
                "object_oids_json" => object_oids_json,
                "keywords_json" => keywords_json,
                "module" => &p.module,
                "status" => &p.status,
                "resolve_oid" => &p.resolve_oid,
                "resolve_values_json" => resolve_values_json,
                "fingerprint_oids_json" => fingerprint_oids_json,
                "severity_oid" => &p.severity_oid,
                "severity_map_json" => severity_map_json,
                "updated_at" => &p.updated_at,
            },
        )?;
        Ok(())
    }

    fn delete_db_sync(pool: &mysql::Pool, id: &str) -> Result<()> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let exists: Option<String> =
            conn.exec_first("SELECT id FROM trap_policies WHERE id = ?", (id,))?;
        if exists.is_none() {
            anyhow::bail!("policy not found: {id}");
        }
        conn.exec_drop("DELETE FROM trap_policies WHERE id = ?", (id,))?;
        Ok(())
    }

    fn clear_all_sync(pool: &mysql::Pool) -> Result<()> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        conn.query_drop("DELETE FROM trap_policies")?;
        Ok(())
    }

    pub async fn reload_from_db(&self) -> Result<()> {
        let pool = self.pool.clone();
        let policies = tokio::task::spawn_blocking(move || Self::load_all_sync(&pool))
            .await
            .context("join load policies")??;
        let pool2 = self.pool.clone();
        let stamp = tokio::task::spawn_blocking(move || Self::max_updated_sync(&pool2))
            .await
            .context("join stamp")??;
        let mut guard = self.inner.write().await;
        *guard = policies;
        *self.stamp.write().await = stamp;
        Ok(())
    }

    /// Hot-reload if MySQL MAX(updated_at) changed.
    pub async fn refresh_if_changed(&self) -> Result<bool> {
        let pool = self.pool.clone();
        let stamp = tokio::task::spawn_blocking(move || Self::max_updated_sync(&pool))
            .await
            .context("join stamp")??;
        if stamp == *self.stamp.read().await {
            return Ok(false);
        }
        self.reload_from_db().await?;
        Ok(true)
    }

    async fn migrate_from_json(&self, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let file: PolicyFile =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        tracing::info!(
            path = %path.display(),
            count = file.policies.len(),
            "migrating trap policies from JSON → MySQL"
        );
        for p in file.policies {
            let pool = self.pool.clone();
            let policy = p.clone();
            tokio::task::spawn_blocking(move || Self::upsert_db_sync(&pool, &policy))
                .await
                .context("join migrate upsert")??;
        }
        self.reload_from_db().await?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<TrapPolicy> {
        self.inner.read().await.clone()
    }

    pub async fn get(&self, id: &str) -> Option<TrapPolicy> {
        self.inner.read().await.iter().find(|p| p.id == id).cloned()
    }

    pub async fn upsert(&self, mut policy: TrapPolicy) -> Result<TrapPolicy> {
        if policy.id.is_empty() {
            policy.id = Uuid::new_v4().to_string();
        }
        if policy.trap_oid.trim().is_empty() {
            anyhow::bail!("trap_oid required");
        }
        if policy.name.trim().is_empty() {
            policy.name = policy.trap_oid.clone();
        }
        if policy.match_mode != "prefix" {
            policy.match_mode = "exact".into();
        }
        policy.resolve_oid = policy.resolve_oid.trim().to_string();
        policy.resolve_values = policy
            .resolve_values
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        policy.fingerprint_oids = policy
            .fingerprint_oids
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        policy.severity_oid = policy.severity_oid.trim().to_string();
        policy.severity_map = policy
            .severity_map
            .into_iter()
            .filter_map(|(k, v)| {
                let k = k.trim().to_string();
                let v = v.trim().to_string();
                if k.is_empty() || v.is_empty() {
                    None
                } else {
                    Some((k, v))
                }
            })
            .collect();
        if policy.resolve_oid.is_empty() && policy.resolve_values.is_empty() {
            if normalize_event_status(&policy.status) == "resolved" {
                policy.status = "resolved".into();
            } else {
                policy.status.clear();
            }
        } else {
            policy.status.clear();
        }
        policy.keywords = policy
            .keywords
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        // Align id with existing OID+mode if present in memory
        {
            let guard = self.inner.read().await;
            if let Some(ex) = guard
                .iter()
                .find(|p| p.trap_oid == policy.trap_oid && p.match_mode == policy.match_mode)
            {
                if policy.id != ex.id
                    && !guard.iter().any(|p| p.id == policy.id)
                {
                    policy.id = ex.id.clone();
                }
            }
        }
        policy.updated_at = Utc::now().to_rfc3339();

        let pool = self.pool.clone();
        let to_save = policy.clone();
        tokio::task::spawn_blocking(move || Self::upsert_db_sync(&pool, &to_save))
            .await
            .context("join upsert")??;

        let mut guard = self.inner.write().await;
        if let Some(i) = guard.iter().position(|p| p.id == policy.id) {
            guard[i] = policy.clone();
        } else if let Some(i) = guard
            .iter()
            .position(|p| p.trap_oid == policy.trap_oid && p.match_mode == policy.match_mode)
        {
            policy.id = guard[i].id.clone();
            guard[i] = policy.clone();
        } else {
            guard.push(policy.clone());
        }
        *self.stamp.write().await = policy.updated_at.clone();
        Ok(policy)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let pool = self.pool.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || Self::delete_db_sync(&pool, &id_owned))
            .await
            .context("join delete")??;
        let mut guard = self.inner.write().await;
        guard.retain(|p| p.id != id);
        Ok(())
    }

    /// Import drafts (from xlsx / MIB export).
    pub async fn import_drafts(
        &self,
        drafts: Vec<TrapPolicyDraft>,
        mode: ImportMode,
    ) -> Result<ImportResult> {
        let mut created = 0u32;
        let mut updated = 0u32;
        let mut skipped = 0u32;
        if mode == ImportMode::Replace {
            let pool = self.pool.clone();
            tokio::task::spawn_blocking(move || Self::clear_all_sync(&pool))
                .await
                .context("join clear")??;
            self.inner.write().await.clear();
        }
        for d in drafts {
            let match_mode = if d.match_mode == "prefix" {
                "prefix"
            } else {
                "exact"
            };
            let existing = {
                let guard = self.inner.read().await;
                guard
                    .iter()
                    .find(|p| p.trap_oid == d.trap_oid && p.match_mode == match_mode)
                    .map(|p| p.id.clone())
            };
            if mode == ImportMode::SkipExisting && existing.is_some() {
                skipped += 1;
                continue;
            }
            let was_update = existing.is_some();
            let policy = draft_to_policy(d, existing);
            self.upsert(policy).await?;
            if was_update {
                updated += 1;
            } else {
                created += 1;
            }
        }
        Ok(ImportResult {
            created,
            updated,
            skipped,
            total: self.inner.read().await.len() as u32,
        })
    }

    /// Import MIB export JSON or `{ policies: [...] }` / bare array.
    pub async fn import_json(&self, raw: &[u8], mode: ImportMode) -> Result<ImportResult> {
        let drafts = parse_import_payload(raw)?;
        self.import_drafts(drafts, mode).await
    }

    pub async fn import_xlsx(&self, raw: &[u8], mode: ImportMode) -> Result<ImportResult> {
        let drafts = crate::policy_xlsx::read_xlsx_drafts(raw)?;
        self.import_drafts(drafts, mode).await
    }

    pub async fn export_file(&self) -> PolicyExport {
        let policies = self.list().await;
        let mut modules: Vec<String> = policies
            .iter()
            .map(|p| p.module.clone())
            .filter(|m| !m.is_empty())
            .collect();
        modules.sort();
        modules.dedup();
        PolicyExport {
            version: 1,
            kind: "eventide-trap-policies".into(),
            exported_at: Utc::now().to_rfc3339(),
            modules,
            policies: policies.into_iter().map(policy_to_draft).collect(),
        }
    }

    pub async fn export_xlsx(&self) -> Result<Vec<u8>> {
        let policies = self.list().await;
        crate::policy_xlsx::write_xlsx_from_policies(&policies)
    }

    /// Best matching enabled policy for a trap (OID match; one policy per OID).
    pub async fn match_trap(&self, trap: &ParsedTrap) -> Option<TrapPolicy> {
        self.match_oid_with_varbinds(&trap.trap_oid, &trap.varbinds)
            .await
    }

    /// Best matching enabled policy for a trap OID.
    pub async fn match_oid(&self, trap_oid: &str) -> Option<TrapPolicy> {
        self.match_oid_with_varbinds(trap_oid, &BTreeMap::new())
            .await
    }

    pub async fn match_oid_with_varbinds(
        &self,
        trap_oid: &str,
        _varbinds: &BTreeMap<String, String>,
    ) -> Option<TrapPolicy> {
        let guard = self.inner.read().await;
        let mut best: Option<&TrapPolicy> = None;
        for p in guard.iter().filter(|p| p.enabled) {
            if !oid_matches(p, trap_oid) {
                continue;
            }
            best = Some(match best {
                None => p,
                Some(cur) => pick_better_oid_policy(cur, p),
            });
        }
        best.cloned()
    }
}

fn policy_from_row(row: mysql::Row) -> Result<TrapPolicy> {
    let id = crate::db::row_string(&row, "id");
    if id.is_empty() {
        anyhow::bail!("policy row missing id");
    }
    let objects_json = crate::db::row_string(&row, "objects_json");
    let object_oids_json = crate::db::row_string(&row, "object_oids_json");
    let keywords_json = crate::db::row_string(&row, "keywords_json");
    let resolve_values_json = crate::db::row_string(&row, "resolve_values_json");
    let fingerprint_oids_json = crate::db::row_string(&row, "fingerprint_oids_json");
    let severity_map_json = crate::db::row_string(&row, "severity_map_json");
    let enabled = match row.get_opt::<i8, _>("enabled") {
        Some(Ok(v)) => v,
        _ => 1,
    };
    Ok(TrapPolicy {
        id,
        name: crate::db::row_string(&row, "name"),
        trap_oid: crate::db::row_string(&row, "trap_oid"),
        match_mode: {
            let m = crate::db::row_string(&row, "match_mode");
            if m.is_empty() {
                "exact".into()
            } else {
                m
            }
        },
        severity: {
            let s = crate::db::row_string(&row, "severity");
            if s.is_empty() {
                "warning".into()
            } else {
                s
            }
        },
        enabled: enabled != 0,
        summary_template: crate::db::row_string(&row, "summary_template"),
        description: crate::db::row_string(&row, "description"),
        objects: serde_json::from_str(if objects_json.is_empty() {
            "[]"
        } else {
            &objects_json
        })
        .unwrap_or_default(),
        object_oids: serde_json::from_str(if object_oids_json.is_empty() {
            "{}"
        } else {
            &object_oids_json
        })
        .unwrap_or_default(),
        keywords: serde_json::from_str(if keywords_json.is_empty() {
            "[]"
        } else {
            &keywords_json
        })
        .unwrap_or_default(),
        module: crate::db::row_string(&row, "module"),
        status: crate::db::row_string(&row, "status"),
        resolve_oid: crate::db::row_string(&row, "resolve_oid"),
        resolve_values: serde_json::from_str(if resolve_values_json.is_empty() {
            "[]"
        } else {
            &resolve_values_json
        })
        .unwrap_or_default(),
        fingerprint_oids: serde_json::from_str(if fingerprint_oids_json.is_empty() {
            "[]"
        } else {
            &fingerprint_oids_json
        })
        .unwrap_or_default(),
        severity_oid: crate::db::row_string(&row, "severity_oid"),
        severity_map: serde_json::from_str(if severity_map_json.is_empty() {
            "{}"
        } else {
            &severity_map_json
        })
        .unwrap_or_default(),
        updated_at: crate::db::row_string(&row, "updated_at"),
    })
}

fn oid_matches(p: &TrapPolicy, trap_oid: &str) -> bool {
    match p.match_mode.as_str() {
        "prefix" => {
            !p.trap_oid.is_empty()
                && (trap_oid == p.trap_oid || trap_oid.starts_with(&format!("{}.", p.trap_oid)))
        }
        _ => trap_oid == p.trap_oid,
    }
}

fn pick_better_oid_policy<'a>(cur: &'a TrapPolicy, p: &'a TrapPolicy) -> &'a TrapPolicy {
    let cur_exact = cur.match_mode != "prefix";
    let p_exact = p.match_mode != "prefix";
    if p_exact && !cur_exact {
        p
    } else if cur_exact && !p_exact {
        cur
    } else if p.trap_oid.len() > cur.trap_oid.len() {
        p
    } else {
        cur
    }
}

/// Decide ingress `firing` / `resolved` from policy + trap varbinds.
/// Default is firing; recovery when resolve_oid's value is in resolve_values.
pub fn resolve_event_status(policy: &TrapPolicy, trap: &ParsedTrap) -> &'static str {
    let values: Vec<String> = policy
        .resolve_values
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    let oid_key = policy.resolve_oid.trim();

    if !oid_key.is_empty() && !values.is_empty() {
        if let Some(actual) = lookup_varbind_value(trap, policy, oid_key) {
            let actual_l = actual.to_ascii_lowercase();
            if values
                .iter()
                .any(|v| v.eq_ignore_ascii_case(&actual_l) || v.to_ascii_lowercase() == actual_l)
            {
                return "resolved";
            }
        }
        return "firing";
    }

    // resolve_values only: any varbind value matches → resolved
    if oid_key.is_empty() && !values.is_empty() {
        for val in trap.varbinds.values() {
            let actual_l = val.to_ascii_lowercase();
            if values
                .iter()
                .any(|v| v.eq_ignore_ascii_case(val) || v.to_ascii_lowercase() == actual_l)
            {
                return "resolved";
            }
        }
        return "firing";
    }

    // Legacy: dedicated recovery trap OID (e.g. linkUp)
    if normalize_event_status(&policy.status) == "resolved" {
        return "resolved";
    }
    "firing"
}

/// Map a numeric varbind OID (with optional instance suffix) → OBJECTS symbolic name.
/// Prefers the longest matching base OID from `object_oids` (name → base OID).
pub fn resolve_object_name(oid: &str, object_oids: &BTreeMap<String, String>) -> Option<String> {
    let oid = oid.trim();
    if oid.is_empty() || object_oids.is_empty() {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    for (name, base) in object_oids {
        let base = base.trim();
        if base.is_empty() {
            continue;
        }
        if oid == base || oid.starts_with(&format!("{base}.")) {
            let len = base.len();
            if best.map(|(l, _)| len > l).unwrap_or(true) {
                best = Some((len, name.as_str()));
            }
        }
    }
    best.map(|(_, n)| n.to_string())
}

/// Look up a varbind by numeric OID or OBJECTS symbolic name (supports instance suffix).
pub fn lookup_varbind_value<'a>(
    trap: &'a ParsedTrap,
    policy: &TrapPolicy,
    key: &str,
) -> Option<&'a str> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    if let Some(v) = trap.varbinds.get(key) {
        return Some(v.as_str());
    }
    if let Some(oid) = policy.object_oids.get(key) {
        if let Some(v) = trap.varbinds.get(oid) {
            return Some(v.as_str());
        }
        if let Some((_, v)) = trap
            .varbinds
            .iter()
            .find(|(k, _)| *k == oid || k.starts_with(&format!("{oid}.")))
        {
            return Some(v.as_str());
        }
    }
    if let Some((_, v)) = trap
        .varbinds
        .iter()
        .find(|(k, _)| *k == key || k.starts_with(&format!("{key}.")))
    {
        return Some(v.as_str());
    }
    None
}

/// Build alert fingerprint. With `fingerprint_oids`, uses those varbind **values**
/// so alarm/recovery with the same serial clear each other.
pub fn build_fingerprint(trap: &ParsedTrap, policy: Option<&TrapPolicy>) -> String {
    if let Some(p) = policy {
        let keys: Vec<&str> = p
            .fingerprint_oids
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !keys.is_empty() {
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    lookup_varbind_value(trap, p, k)
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            return format!("{}|{}|{}", trap.peer_ip, trap.trap_oid, parts.join("|"));
        }
    }
    let mut oid_keys: Vec<&str> = trap
        .varbinds
        .keys()
        .map(|s| s.as_str())
        .filter(|oid| *oid != "1.3.6.1.6.3.1.1.4.1.0" && *oid != "1.3.6.1.2.1.1.3.0")
        .collect();
    oid_keys.sort_unstable();
    format!("{}|{}|{}", trap.peer_ip, trap.trap_oid, oid_keys.join(","))
}

/// Dynamic severity from severity_oid + severity_map; else policy/default severity.
pub fn resolve_severity(policy: &TrapPolicy, trap: &ParsedTrap) -> String {
    let oid = policy.severity_oid.trim();
    if !oid.is_empty() && !policy.severity_map.is_empty() {
        if let Some(raw) = lookup_varbind_value(trap, policy, oid) {
            let raw_t = raw.trim();
            if let Some(mapped) = policy.severity_map.get(raw_t) {
                return mapped.clone();
            }
            // case-insensitive key match
            let raw_l = raw_t.to_ascii_lowercase();
            for (k, v) in &policy.severity_map {
                if k.eq_ignore_ascii_case(&raw_l) {
                    return v.clone();
                }
            }
        }
    }
    if policy.severity.is_empty() {
        "warning".into()
    } else {
        policy.severity.clone()
    }
}

/// Normalize legacy status tokens to `firing` or `resolved`.
pub fn normalize_event_status(raw: &str) -> &'static str {
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "resolved" | "resolve" | "recover" | "recovery" | "clear" | "cleared" | "ok"
        | "normal" | "good" => "resolved",
        "" | "firing" | "fire" | "alarm" | "alert" | "current" | "deprecated" | "obsolete"
        | "mandatory" | "optional" => "firing",
        other => {
            if other.contains("resolv")
                || other.contains("recover")
                || other.contains("clear")
            {
                "resolved"
            } else {
                "firing"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Same OID → update; new OID → create.
    Merge,
    /// Same OID → skip; new OID → create.
    SkipExisting,
    /// Clear all then import (legacy).
    Replace,
}

impl ImportMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "skip" | "skip_existing" | "ignore" => Self::SkipExisting,
            "replace" | "clear" => Self::Replace,
            _ => Self::Merge,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
    #[serde(default)]
    pub skipped: u32,
    pub total: u32,
}

fn draft_to_policy(d: TrapPolicyDraft, id: Option<String>) -> TrapPolicy {
    let resolve_oid = d.resolve_oid.trim().to_string();
    let resolve_values: Vec<String> = d
        .resolve_values
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    // Old Excel「状态」列若仍是 firing/resolved，且无恢复条件 → 作为始终恢复标记
    let status = if resolve_oid.is_empty() && resolve_values.is_empty() {
        if normalize_event_status(&d.status) == "resolved" {
            "resolved".into()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    TrapPolicy {
        id: id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        name: d.name,
        trap_oid: d.trap_oid,
        match_mode: if d.match_mode == "prefix" {
            "prefix".into()
        } else {
            "exact".into()
        },
        severity: if d.severity.is_empty() {
            "warning".into()
        } else {
            d.severity
        },
        enabled: d.enabled,
        summary_template: d.summary_template,
        description: d.description,
        objects: d.objects,
        object_oids: d.object_oids,
        keywords: d.keywords,
        module: d.module,
        status,
        resolve_oid,
        resolve_values,
        fingerprint_oids: d
            .fingerprint_oids
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect(),
        severity_oid: d.severity_oid.trim().to_string(),
        severity_map: d
            .severity_map
            .into_iter()
            .filter_map(|(k, v)| {
                let k = k.trim().to_string();
                let v = v.trim().to_string();
                if k.is_empty() || v.is_empty() {
                    None
                } else {
                    Some((k, v))
                }
            })
            .collect(),
        updated_at: Utc::now().to_rfc3339(),
    }
}

fn policy_to_draft(p: TrapPolicy) -> TrapPolicyDraft {
    TrapPolicyDraft {
        name: p.name,
        trap_oid: p.trap_oid,
        match_mode: p.match_mode,
        severity: p.severity,
        enabled: p.enabled,
        summary_template: p.summary_template,
        description: p.description,
        objects: p.objects,
        object_oids: p.object_oids,
        keywords: p.keywords,
        module: p.module,
        status: p.status,
        resolve_oid: p.resolve_oid,
        resolve_values: p.resolve_values,
        fingerprint_oids: p.fingerprint_oids,
        severity_oid: p.severity_oid,
        severity_map: p.severity_map,
    }
}

fn parse_import_payload(raw: &[u8]) -> Result<Vec<TrapPolicyDraft>> {
    let v: serde_json::Value = serde_json::from_slice(raw).context("invalid JSON")?;
    if let Some(arr) = v.as_array() {
        return Ok(serde_json::from_value(serde_json::Value::Array(arr.clone()))?);
    }
    if let Some(arr) = v.get("policies").and_then(|x| x.as_array()) {
        return Ok(serde_json::from_value(serde_json::Value::Array(arr.clone()))?);
    }
    // single draft object
    if v.get("trap_oid").is_some() {
        let one: TrapPolicyDraft = serde_json::from_value(v)?;
        return Ok(vec![one]);
    }
    anyhow::bail!("expected PolicyExport / {{policies:[]}} / policy array");
}

/// Render `${var}` template. Unknown vars left as empty string.
pub fn render_template(template: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = template[i + 2..].find('}') {
                let key = &template[i + 2..i + 2 + end];
                out.push_str(vars.get(key).map(|s| s.as_str()).unwrap_or(""));
                i = i + 2 + end + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn build_template_vars(trap: &ParsedTrap, policy: &TrapPolicy) -> BTreeMap<String, String> {
    let mut vars = BTreeMap::new();
    vars.insert("alertname".into(), policy.name.clone());
    vars.insert("name".into(), policy.name.clone());
    vars.insert("ip".into(), trap.peer_ip.clone());
    vars.insert("trap_oid".into(), trap.trap_oid.clone());
    vars.insert("severity".into(), policy.severity.clone());
    vars.insert("description".into(), policy.description.clone());
    vars.insert("module".into(), policy.module.clone());
    vars.insert("community".into(), trap.community.clone());

    for (i, (oid, val)) in trap.varbinds.iter().enumerate() {
        vars.insert(format!("vb{i}"), val.clone());
        vars.insert(format!("vb{i}_oid"), oid.clone());
        vars.insert(oid.clone(), val.clone());
    }

    for (name, oid) in &policy.object_oids {
        if let Some(val) = trap.varbinds.get(oid) {
            vars.insert(name.clone(), val.clone());
            continue;
        }
        // instance OID: prefix match oid.
        if let Some((_, val)) = trap.varbinds.iter().find(|(k, _)| {
            *k == oid || k.starts_with(&format!("{oid}."))
        }) {
            vars.insert(name.clone(), val.clone());
        }
    }

    // Fallback: object name as key if already a value somehow
    for name in &policy.objects {
        if !vars.contains_key(name) {
            vars.insert(name.clone(), String::new());
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_render() {
        let mut vars = BTreeMap::new();
        vars.insert("alertname".into(), "linkDown".into());
        vars.insert("ip".into(), "10.0.0.1".into());
        assert_eq!(
            render_template("${alertname} from ${ip}", &vars),
            "linkDown from 10.0.0.1"
        );
    }

    #[test]
    fn event_status_aliases() {
        assert_eq!(normalize_event_status("resolved"), "resolved");
        assert_eq!(normalize_event_status("CLEAR"), "resolved");
        assert_eq!(normalize_event_status("current"), "firing");
        assert_eq!(normalize_event_status(""), "firing");
    }

    #[test]
    fn resolve_by_oid_value() {
        let mut vbs = BTreeMap::new();
        vbs.insert("1.3.6.1.4.1.9.9.41.1.2.3.0".into(), "cleared".into());
        let trap = ParsedTrap {
            version: "v2c".into(),
            community: "public".into(),
            peer_ip: "10.0.0.1".into(),
            peer_port: 0,
            trap_oid: "1.3.6.1.4.1.9.9.41.2.0.1".into(),
            alertname: "t".into(),
            severity_hint: None,
            varbinds: vbs,
            raw_note: "".into(),
        };
        let mut policy = TrapPolicy {
            id: "1".into(),
            name: "cisco".into(),
            trap_oid: trap.trap_oid.clone(),
            match_mode: "exact".into(),
            severity: "warning".into(),
            enabled: true,
            summary_template: "".into(),
            description: "".into(),
            objects: vec![],
            object_oids: BTreeMap::new(),
            keywords: vec![],
            module: "".into(),
            status: "".into(),
            resolve_oid: "1.3.6.1.4.1.9.9.41.1.2.3".into(),
            resolve_values: vec!["cleared".into(), "0".into()],
            fingerprint_oids: vec![],
            severity_oid: "".into(),
            severity_map: BTreeMap::new(),
            updated_at: "".into(),
        };
        assert_eq!(resolve_event_status(&policy, &trap), "resolved");
        policy.resolve_values = vec!["active".into()];
        assert_eq!(resolve_event_status(&policy, &trap), "firing");
    }

    #[test]
    fn fingerprint_and_severity_from_varbinds() {
        let mut vbs = BTreeMap::new();
        vbs.insert(
            "1.3.6.1.4.1.2011.2.91.10.3.1.1.9.0".into(),
            "SN-10086".into(),
        );
        vbs.insert("1.3.6.1.4.1.2011.2.91.10.3.1.1.6.0".into(), "2".into());
        vbs.insert("1.3.6.1.4.1.2011.2.91.10.3.1.1.11.0".into(), "1".into());
        let trap = ParsedTrap {
            version: "v2c".into(),
            community: "public".into(),
            peer_ip: "10.1.1.1".into(),
            peer_port: 0,
            trap_oid: "1.3.6.1.4.1.2011.2.91.10.2.1.0.1".into(),
            alertname: "hw".into(),
            severity_hint: None,
            varbinds: vbs,
            raw_note: "".into(),
        };
        let policy = TrapPolicy {
            id: "1".into(),
            name: "huawei".into(),
            trap_oid: trap.trap_oid.clone(),
            match_mode: "exact".into(),
            severity: "warning".into(),
            enabled: true,
            summary_template: "".into(),
            description: "".into(),
            objects: vec![],
            object_oids: BTreeMap::new(),
            keywords: vec![],
            module: "".into(),
            status: "".into(),
            resolve_oid: "1.3.6.1.4.1.2011.2.91.10.3.1.1.11".into(),
            resolve_values: vec!["2".into()],
            fingerprint_oids: vec!["1.3.6.1.4.1.2011.2.91.10.3.1.1.9".into()],
            severity_oid: "1.3.6.1.4.1.2011.2.91.10.3.1.1.6".into(),
            severity_map: BTreeMap::from([
                ("1".into(), "disaster".into()),
                ("2".into(), "high".into()),
                ("3".into(), "average".into()),
                ("4".into(), "warning".into()),
            ]),
            updated_at: "".into(),
        };
        assert_eq!(resolve_event_status(&policy, &trap), "firing");
        assert_eq!(resolve_severity(&policy, &trap), "high");
        assert_eq!(
            build_fingerprint(&trap, Some(&policy)),
            "10.1.1.1|1.3.6.1.4.1.2011.2.91.10.2.1.0.1|SN-10086"
        );
    }
}
