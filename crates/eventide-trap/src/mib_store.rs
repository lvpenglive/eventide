//! On-disk MIB library + resolved OID browser / trap-policy export.

use anyhow::{Context, Result};
use chrono::Utc;
use mib_rs::{DiagnosticConfig, Loader, ResolverStrictness};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const MIB_EXTS: &[&str] = &["mib", "txt", "my", "smi"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MibEntry {
    /// Stable id = module name (unique in store).
    pub id: String,
    pub filename: String,
    pub module_name: String,
    pub uploaded_at: String,
    pub size: u64,
    pub parse_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub notification_count: u32,
    pub node_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_oid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreeNode {
    pub oid: String,
    pub name: String,
    pub arc: u32,
    pub kind: String,
    pub has_children: bool,
    pub is_notification: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeDetail {
    pub oid: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    pub is_notification: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objects: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapPolicyDraft {
    pub name: String,
    pub trap_oid: String,
    pub match_mode: String,
    pub severity: String,
    pub enabled: bool,
    pub summary_template: String,
    pub description: String,
    pub objects: Vec<String>,
    /// name → numeric OID (filled when exporting from resolved MIB).
    #[serde(default)]
    pub object_oids: BTreeMap<String, String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub module: String,
    /// Legacy always-resolved marker when no resolve_oid/values.
    #[serde(default)]
    pub status: String,
    /// Varbind OID or OBJECTS name for recovery check.
    #[serde(default)]
    pub resolve_oid: String,
    /// Values of resolve_oid that mean recovery.
    #[serde(default)]
    pub resolve_values: Vec<String>,
    #[serde(default)]
    pub fingerprint_oids: Vec<String>,
    #[serde(default)]
    pub severity_oid: String,
    #[serde(default)]
    pub severity_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyExport {
    pub version: u32,
    pub kind: String,
    pub exported_at: String,
    pub modules: Vec<String>,
    pub policies: Vec<TrapPolicyDraft>,
}

struct Catalog {
    entries: Vec<MibEntry>,
    mib: Option<Arc<mib_rs::Mib>>,
    load_error: Option<String>,
}

pub struct MibStore {
    pool: mysql::Pool,
    s3: Arc<crate::s3_store::S3Store>,
    cache_dir: PathBuf,
    backend: String,
    inner: RwLock<Catalog>,
    stamp: RwLock<String>,
}

#[derive(Clone)]
struct MibRow {
    id: String,
    filename: String,
    object_key: String,
    size: i64,
    uploaded_at: String,
    parse_ok: i8,
    error: Option<String>,
    notification_count: i32,
    node_count: i32,
    module_oid: Option<String>,
    updated_at: String,
}

impl MibStore {
    pub async fn open(
        pool: mysql::Pool,
        s3: Arc<crate::s3_store::S3Store>,
        cache_dir: impl Into<PathBuf>,
        migrate_from: Option<&Path>,
    ) -> Result<Self> {
        crate::db::ensure_trap_schema(&pool)?;
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)
            .with_context(|| format!("create mib cache {}", cache_dir.display()))?;
        let bucket = s3.bucket().to_string();
        let store = Self {
            pool,
            s3,
            cache_dir,
            backend: format!("s3:{bucket}/mibs + mysql:trap_mibs"),
            inner: RwLock::new(Catalog {
                entries: Vec::new(),
                mib: None,
                load_error: None,
            }),
            stamp: RwLock::new(String::new()),
        };
        store.reload().await?;
        if store.inner.read().await.entries.is_empty() {
            if let Some(dir) = migrate_from {
                if dir.is_dir() {
                    store.migrate_from_dir(dir).await?;
                }
            }
        }
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn backend(&self) -> &str {
        &self.backend
    }

    fn list_rows_sync(pool: &mysql::Pool) -> Result<Vec<MibRow>> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let mut result = conn
            .query_iter(
                r#"SELECT id, filename, object_key, size, uploaded_at, parse_ok, error,
                          notification_count, node_count, module_oid, updated_at
                   FROM trap_mibs ORDER BY id"#,
            )
            .context("select trap_mibs")?;
        let mut out = Vec::new();
        for row in result.by_ref() {
            let row = row.context("mib row")?;
            out.push(MibRow {
                id: crate::db::row_string(&row, "id"),
                filename: crate::db::row_string(&row, "filename"),
                object_key: crate::db::row_string(&row, "object_key"),
                size: crate::db::row_i64(&row, "size"),
                uploaded_at: crate::db::row_string(&row, "uploaded_at"),
                parse_ok: crate::db::row_i8(&row, "parse_ok"),
                error: crate::db::row_string_opt(&row, "error"),
                notification_count: crate::db::row_i32(&row, "notification_count"),
                node_count: crate::db::row_i32(&row, "node_count"),
                module_oid: crate::db::row_string_opt(&row, "module_oid"),
                updated_at: crate::db::row_string(&row, "updated_at"),
            });
        }
        Ok(out)
    }

    fn max_updated_sync(pool: &mysql::Pool) -> Result<String> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let v: Option<Option<String>> =
            conn.exec_first("SELECT MAX(updated_at) FROM trap_mibs", ())?;
        Ok(v.flatten().unwrap_or_default())
    }

    fn upsert_row_sync(pool: &mysql::Pool, e: &MibEntry, object_key: &str) -> Result<()> {
        use mysql::params;
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let now = Utc::now().to_rfc3339();
        conn.exec_drop(
            r#"INSERT INTO trap_mibs (
                id, filename, object_key, size, uploaded_at, parse_ok, error,
                notification_count, node_count, module_oid, updated_at
            ) VALUES (
                :id, :filename, :object_key, :size, :uploaded_at, :parse_ok, :error,
                :notification_count, :node_count, :module_oid, :updated_at
            )
            ON DUPLICATE KEY UPDATE
                filename=VALUES(filename), object_key=VALUES(object_key), size=VALUES(size),
                uploaded_at=VALUES(uploaded_at), parse_ok=VALUES(parse_ok), error=VALUES(error),
                notification_count=VALUES(notification_count), node_count=VALUES(node_count),
                module_oid=VALUES(module_oid), updated_at=VALUES(updated_at)"#,
            params! {
                "id" => &e.id,
                "filename" => &e.filename,
                "object_key" => object_key,
                "size" => e.size as i64,
                "uploaded_at" => &e.uploaded_at,
                "parse_ok" => if e.parse_ok { 1i8 } else { 0i8 },
                "error" => &e.error,
                "notification_count" => e.notification_count as i32,
                "node_count" => e.node_count as i32,
                "module_oid" => &e.module_oid,
                "updated_at" => now,
            },
        )?;
        Ok(())
    }

    fn delete_row_sync(pool: &mysql::Pool, id: &str) -> Result<Option<MibRow>> {
        use mysql::prelude::*;
        let mut conn = crate::db::conn(pool)?;
        let mut result = conn
            .exec_iter(
                r#"SELECT id, filename, object_key, size, uploaded_at, parse_ok, error,
                          notification_count, node_count, module_oid, updated_at
                   FROM trap_mibs WHERE id = ?"#,
                (id,),
            )
            .context("select mib for delete")?;
        let mut found = None;
        for row in result.by_ref() {
            let row = row.context("mib row")?;
            found = Some(MibRow {
                id: crate::db::row_string(&row, "id"),
                filename: crate::db::row_string(&row, "filename"),
                object_key: crate::db::row_string(&row, "object_key"),
                size: crate::db::row_i64(&row, "size"),
                uploaded_at: crate::db::row_string(&row, "uploaded_at"),
                parse_ok: crate::db::row_i8(&row, "parse_ok"),
                error: crate::db::row_string_opt(&row, "error"),
                notification_count: crate::db::row_i32(&row, "notification_count"),
                node_count: crate::db::row_i32(&row, "node_count"),
                module_oid: crate::db::row_string_opt(&row, "module_oid"),
                updated_at: crate::db::row_string(&row, "updated_at"),
            });
            break;
        }
        drop(result);
        if found.is_none() {
            return Ok(None);
        }
        conn.exec_drop("DELETE FROM trap_mibs WHERE id = ?", (id,))?;
        Ok(found)
    }

    async fn sync_cache(&self, rows: &[MibRow]) -> Result<()> {
        for r in rows {
            let path = self.cache_dir.join(&r.filename);
            let need = match std::fs::metadata(&path) {
                Ok(m) => m.len() as i64 != r.size,
                Err(_) => true,
            };
            if need {
                let bytes = self.s3.get_object(&r.object_key).await?;
                std::fs::write(&path, &bytes)
                    .with_context(|| format!("write cache {}", path.display()))?;
            }
        }
        // Remove cache files not in DB
        if let Ok(rd) = std::fs::read_dir(&self.cache_dir) {
            let keep: std::collections::HashSet<&str> =
                rows.iter().map(|r| r.filename.as_str()).collect();
            for ent in rd.flatten() {
                let path = ent.path();
                if !path.is_file() {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if !keep.contains(name) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(())
    }

    pub async fn list(&self) -> Vec<MibEntry> {
        self.inner.read().await.entries.clone()
    }

    pub async fn load_error(&self) -> Option<String> {
        self.inner.read().await.load_error.clone()
    }

    pub async fn get(&self, id: &str) -> Option<MibEntry> {
        self.inner
            .read()
            .await
            .entries
            .iter()
            .find(|e| e.id == id || e.module_name == id)
            .cloned()
    }

    pub async fn refresh_if_changed(&self) -> Result<bool> {
        let pool = self.pool.clone();
        let stamp = tokio::task::spawn_blocking(move || Self::max_updated_sync(&pool))
            .await
            .context("join mib stamp")??;
        if stamp == *self.stamp.read().await {
            return Ok(false);
        }
        self.reload().await?;
        Ok(true)
    }

    pub async fn reload(&self) -> Result<()> {
        let pool = self.pool.clone();
        let rows = tokio::task::spawn_blocking(move || Self::list_rows_sync(&pool))
            .await
            .context("join list mibs")??;
        self.sync_cache(&rows).await?;

        let (mib, load_error) = if rows.is_empty() {
            (None, None)
        } else {
            match load_dir(&self.cache_dir) {
                Ok(m) => (Some(Arc::new(m)), None),
                Err(e) => (None, Some(e.to_string())),
            }
        };

        let mut entries = Vec::new();
        for r in &rows {
            let mut entry = MibEntry {
                id: r.id.clone(),
                filename: r.filename.clone(),
                module_name: r.id.clone(),
                uploaded_at: r.uploaded_at.clone(),
                size: r.size as u64,
                parse_ok: false,
                error: load_error.clone().or_else(|| r.error.clone()),
                notification_count: 0,
                node_count: 0,
                module_oid: None,
            };
            if let Some(ref m) = mib {
                if let Some(module) = m.module(&r.id) {
                    entry.parse_ok = true;
                    entry.error = None;
                    entry.module_oid = module.oid().map(|o| o.to_string());
                    entry.node_count = module.nodes().count() as u32;
                    entry.notification_count = m
                        .notifications()
                        .filter(|n| n.module().map(|x| x.name()) == Some(r.id.as_str()))
                        .count() as u32;
                } else {
                    entry.parse_ok = false;
                    entry.error = Some(format!(
                        "模块 `{}` 未出现在解析结果中（可能 IMPORTS 失败或语法错误）",
                        r.id
                    ));
                }
            }
            // Persist parse stats if changed
            if entry.parse_ok != (r.parse_ok != 0)
                || entry.notification_count != r.notification_count as u32
                || entry.node_count != r.node_count as u32
                || entry.module_oid != r.module_oid
                || entry.error != r.error
            {
                let pool = self.pool.clone();
                let e2 = entry.clone();
                let key = r.object_key.clone();
                let _ = tokio::task::spawn_blocking(move || Self::upsert_row_sync(&pool, &e2, &key))
                    .await;
            }
            entries.push(entry);
        }
        entries.sort_by(|a, b| a.module_name.cmp(&b.module_name));

        let pool2 = self.pool.clone();
        let stamp = tokio::task::spawn_blocking(move || Self::max_updated_sync(&pool2))
            .await
            .context("join stamp")??;

        let mut guard = self.inner.write().await;
        guard.entries = entries;
        guard.mib = mib;
        guard.load_error = load_error;
        *self.stamp.write().await = stamp;
        Ok(())
    }

    async fn migrate_from_dir(&self, dir: &Path) -> Result<()> {
        let rd = std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))?;
        let mut n = 0u32;
        for ent in rd.flatten() {
            let path = ent.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !MIB_EXTS.iter().any(|x| *x == ext) {
                continue;
            }
            let bytes = std::fs::read(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown.mib");
            self.upload(filename, &bytes).await?;
            n += 1;
        }
        if n > 0 {
            tracing::info!(dir = %dir.display(), count = n, "migrated MIBs → RustFS + MySQL");
        }
        Ok(())
    }

    pub async fn upload(&self, filename: &str, bytes: &[u8]) -> Result<MibEntry> {
        let safe = sanitize_filename(filename);
        if safe.is_empty() {
            anyhow::bail!("invalid filename");
        }
        let text = String::from_utf8_lossy(bytes);
        let module_name = extract_module_name(&text).unwrap_or_else(|| {
            Path::new(&safe)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("UNKNOWN")
                .to_string()
        });

        let ext = Path::new(&safe)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mib");
        let out_name = format!("{module_name}.{ext}");
        let object_key = crate::s3_store::object_key(&module_name, &out_name);

        // Remove older object if same module different filename
        {
            let guard = self.inner.read().await;
            for e in &guard.entries {
                if e.module_name == module_name && e.filename != out_name {
                    let old_key = crate::s3_store::object_key(&module_name, &e.filename);
                    let _ = self.s3.delete_object(&old_key).await;
                    let pool = self.pool.clone();
                    let id = e.id.clone();
                    let _ = tokio::task::spawn_blocking(move || Self::delete_row_sync(&pool, &id))
                        .await;
                    let _ = std::fs::remove_file(self.cache_dir.join(&e.filename));
                }
            }
        }

        self.s3.put_object(&object_key, bytes).await?;
        let cache_path = self.cache_dir.join(&out_name);
        std::fs::write(&cache_path, bytes)
            .with_context(|| format!("write {}", cache_path.display()))?;

        let entry = MibEntry {
            id: module_name.clone(),
            filename: out_name.clone(),
            module_name: module_name.clone(),
            uploaded_at: Utc::now().to_rfc3339(),
            size: bytes.len() as u64,
            parse_ok: false,
            error: None,
            notification_count: 0,
            node_count: 0,
            module_oid: None,
        };
        let pool = self.pool.clone();
        let e2 = entry.clone();
        let key = object_key.clone();
        tokio::task::spawn_blocking(move || Self::upsert_row_sync(&pool, &e2, &key))
            .await
            .context("join upsert mib")??;

        self.reload().await?;
        self.get(&module_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("upload saved but module not listed"))
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let pool = self.pool.clone();
        let id_owned = id.to_string();
        let row = tokio::task::spawn_blocking(move || Self::delete_row_sync(&pool, &id_owned))
            .await
            .context("join delete mib")??;
        let Some(row) = row else {
            anyhow::bail!("module not found: {id}");
        };
        let _ = self.s3.delete_object(&row.object_key).await;
        let _ = std::fs::remove_file(self.cache_dir.join(&row.filename));
        self.reload().await?;
        Ok(())
    }

    pub async fn children(&self, module: &str, parent_oid: Option<&str>) -> Result<Vec<TreeNode>> {
        let guard = self.inner.read().await;
        let mib = guard
            .mib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!(guard.load_error.clone().unwrap_or_else(|| "MIB 未加载".into())))?;
        let mod_h = mib
            .module(module)
            .ok_or_else(|| anyhow::anyhow!("模块不存在: {module}"))?;

        let nodes: Vec<TreeNode> = if let Some(oid_str) = parent_oid.filter(|s| !s.is_empty()) {
            let oid: mib_rs::Oid = oid_str
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid oid: {e}"))?;
            let parent = mib.lookup_oid(&oid);
            parent
                .children()
                .map(|n| to_tree_node(n))
                .collect()
        } else if let Some(moid) = mod_h.oid() {
            // Module identity as browser root
            let root = mib.lookup_oid(moid);
            vec![to_tree_node(root)]
        } else {
            // Fallback: nodes in module whose parent is outside the module
            mod_h
                .nodes()
                .filter(|n| {
                    n.parent()
                        .map(|p| p.module().map(|m| m.name()) != Some(module))
                        .unwrap_or(true)
                })
                .map(to_tree_node)
                .collect()
        };
        Ok(nodes)
    }

    pub async fn node_detail(&self, module: &str, oid_str: &str) -> Result<NodeDetail> {
        let guard = self.inner.read().await;
        let mib = guard
            .mib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MIB 未加载"))?;
        let _ = mib
            .module(module)
            .ok_or_else(|| anyhow::anyhow!("模块不存在: {module}"))?;
        let oid: mib_rs::Oid = oid_str
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid oid: {e}"))?;
        let node = mib.lookup_oid(&oid);
        let is_notif = node.notification().is_some();
        let objects = node.notification().map(|n| {
            n.objects()
                .map(|o| o.name().to_string())
                .collect::<Vec<_>>()
        });
        Ok(NodeDetail {
            oid: node.oid().to_string(),
            name: node.name().to_string(),
            kind: format!("{:?}", node.kind()),
            description: node.description().to_string(),
            status: node
                .status()
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| "—".into()),
            module: node.module().map(|m| m.name().to_string()),
            is_notification: is_notif,
            objects,
        })
    }

    pub async fn notifications(&self, module: &str) -> Result<Vec<TrapPolicyDraft>> {
        Ok(self.export_policies(Some(module)).await?.policies)
    }

    pub async fn export_policies(&self, module_filter: Option<&str>) -> Result<PolicyExport> {
        let guard = self.inner.read().await;
        let mib = guard
            .mib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!(guard.load_error.clone().unwrap_or_else(|| "MIB 未加载".into())))?;

        let allowed: std::collections::HashSet<String> = guard
            .entries
            .iter()
            .filter(|e| e.parse_ok)
            .map(|e| e.module_name.clone())
            .collect();

        let mut policies = Vec::new();
        let mut modules = Vec::new();
        for notif in mib.notifications() {
            let mod_name = notif
                .module()
                .map(|m| m.name().to_string())
                .unwrap_or_else(|| "UNKNOWN".into());
            if !allowed.contains(&mod_name) {
                continue;
            }
            if let Some(f) = module_filter {
                if mod_name != f {
                    continue;
                }
            }
            if !modules.iter().any(|m| m == &mod_name) {
                modules.push(mod_name.clone());
            }
            let trap_oid = notif
                .node()
                .map(|n| n.oid().to_string())
                .unwrap_or_default();
            if trap_oid.is_empty() {
                continue;
            }
            let name = notif.name().to_string();
            let description = notif.description().to_string();
            let mut objects = Vec::new();
            let mut object_oids = BTreeMap::new();
            for o in notif.objects() {
                objects.push(o.name().to_string());
                object_oids.insert(o.name().to_string(), o.node().oid().to_string());
            }
            let severity = heuristic_severity(&name, &description);
            let status = heuristic_event_status(&name, &description);
            let object_vars: String = objects
                .iter()
                .map(|o| format!("${{{o}}}"))
                .collect::<Vec<_>>()
                .join(" ");
            let summary_template = if object_vars.is_empty() {
                format!("${{alertname}}: {}", description.chars().take(120).collect::<String>())
            } else {
                format!("${{alertname}}: {object_vars}")
            };
            policies.push(TrapPolicyDraft {
                name: name.clone(),
                trap_oid,
                match_mode: "exact".into(),
                severity,
                enabled: true,
                summary_template,
                description,
                objects,
                object_oids,
                keywords: vec![],
                module: mod_name,
                status,
                resolve_oid: String::new(),
                resolve_values: vec![],
                fingerprint_oids: vec![],
                severity_oid: String::new(),
                severity_map: BTreeMap::new(),
            });
        }
        policies.sort_by(|a, b| a.trap_oid.cmp(&b.trap_oid));
        modules.sort();
        Ok(PolicyExport {
            version: 1,
            kind: "eventide-trap-policies".into(),
            exported_at: Utc::now().to_rfc3339(),
            modules,
            policies,
        })
    }
}

fn load_dir(dir: &Path) -> Result<mib_rs::Mib> {
    let src = mib_rs::source::dir(dir).with_context(|| format!("mib source {}", dir.display()))?;
    Loader::new()
        .source(src)
        .resolver_strictness(ResolverStrictness::Permissive)
        .diagnostic_config(DiagnosticConfig::silent())
        .load()
        .map_err(|e| anyhow::anyhow!("load MIBs: {e}"))
}

fn to_tree_node(n: mib_rs::Node<'_>) -> TreeNode {
    let has_children = n.children().next().is_some();
    TreeNode {
        oid: n.oid().to_string(),
        name: n.name().to_string(),
        arc: n.arc(),
        kind: format!("{:?}", n.kind()),
        has_children,
        is_notification: n.notification().is_some(),
        module: n.module().map(|m| m.name().to_string()),
    }
}

fn extract_module_name(content: &str) -> Option<String> {
    // MODULE-NAME DEFINITIONS ::= BEGIN
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("--") {
            continue;
        }
        let upper = t.to_ascii_uppercase();
        if let Some(idx) = upper.find("DEFINITIONS") {
            let name = t[..idx].trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(name.to_string());
            }
        }
        // stop after first non-comment content that isn't DEFINITIONS (IMPORTS etc. later)
        if upper.contains("::=") && !upper.contains("DEFINITIONS") {
            break;
        }
    }
    None
}

fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn heuristic_severity(name: &str, desc: &str) -> String {
    let blob = format!("{name} {desc}").to_ascii_lowercase();
    if blob.contains("critical")
        || blob.contains("disaster")
        || blob.contains("down")
        || blob.contains("fail")
        || blob.contains("fatal")
        || blob.contains("alarm")
    {
        "disaster".into()
    } else if blob.contains("high") || blob.contains("major") {
        "high".into()
    } else if blob.contains("average") || blob.contains("minor") {
        "average".into()
    } else if blob.contains("warn") || blob.contains("degrad") {
        "warning".into()
    } else if looks_like_recover(name, desc) {
        "information".into()
    } else {
        "warning".into()
    }
}

fn heuristic_event_status(name: &str, desc: &str) -> String {
    if looks_like_recover(name, desc) {
        "resolved".into()
    } else {
        "firing".into()
    }
}

fn looks_like_recover(name: &str, desc: &str) -> bool {
    let blob = format!("{name} {desc}").to_ascii_lowercase();
    if blob.contains("recover") || blob.contains("clear") {
        return true;
    }
    let n = name.to_ascii_lowercase();
    (n.ends_with("up") || n.contains("uptrap") || n.contains("linkup"))
        && !n.contains("startup")
        && !n.contains("setup")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_module_name() {
        let s = "FOO-BAR-MIB DEFINITIONS ::= BEGIN\nEND";
        assert_eq!(extract_module_name(s).as_deref(), Some("FOO-BAR-MIB"));
    }
}
