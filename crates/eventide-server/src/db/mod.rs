//! SQLite persistence for Eventide.

mod iam_repo;
mod repo;

use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref()).context("sqlite open")?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS datasources (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    url TEXT NOT NULL,
    options_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notify_channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    url TEXT NOT NULL,
    secret TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    datasource_id TEXT NOT NULL,
    expr TEXT NOT NULL,
    comparator TEXT NOT NULL,
    threshold REAL NOT NULL,
    for_seconds INTEGER NOT NULL DEFAULT 0,
    interval_seconds INTEGER NOT NULL DEFAULT 30,
    severity TEXT NOT NULL,
    labels_json TEXT NOT NULL DEFAULT '{}',
    annotations_json TEXT NOT NULL DEFAULT '{}',
    channel_ids_json TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_run_at TEXT
);

CREATE TABLE IF NOT EXISTS alert_events (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    severity TEXT NOT NULL,
    labels_json TEXT NOT NULL,
    annotations_json TEXT NOT NULL,
    value REAL,
    starts_at TEXT NOT NULL,
    ends_at TEXT,
    pending_since TEXT,
    last_evaluated_at TEXT NOT NULL,
    notified_firing INTEGER NOT NULL DEFAULT 0,
    notified_resolved INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_alert_status ON alert_events(status);
CREATE INDEX IF NOT EXISTS idx_alert_rule ON alert_events(rule_id);

CREATE TABLE IF NOT EXISTS silences (
    id TEXT PRIMARY KEY,
    rule_id TEXT,
    matchers_json TEXT NOT NULL DEFAULT '{}',
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    comment TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notify_logs (
    id TEXT PRIMARY KEY,
    alert_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    transition TEXT NOT NULL,
    success INTEGER NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ingress_routes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    token TEXT,
    endpoint TEXT NOT NULL DEFAULT '',
    options_json TEXT NOT NULL DEFAULT '{}',
    channel_ids_json TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#,
        )?;

        // Drop legacy FKs by rebuilding alert_events once (keeps data).
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 2 {
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS alert_events_v2 (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    severity TEXT NOT NULL,
    labels_json TEXT NOT NULL,
    annotations_json TEXT NOT NULL,
    value REAL,
    starts_at TEXT NOT NULL,
    ends_at TEXT,
    pending_since TEXT,
    last_evaluated_at TEXT NOT NULL,
    notified_firing INTEGER NOT NULL DEFAULT 0,
    notified_resolved INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO alert_events_v2
    SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
           value, starts_at, ends_at, pending_since, last_evaluated_at,
           notified_firing, notified_resolved
    FROM alert_events;
DROP TABLE alert_events;
ALTER TABLE alert_events_v2 RENAME TO alert_events;
CREATE INDEX IF NOT EXISTS idx_alert_status ON alert_events(status);
CREATE INDEX IF NOT EXISTS idx_alert_rule ON alert_events(rule_id);
INSERT INTO schema_meta(key, value) VALUES('version', '2')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v3: datasource/ingress options + kafka offset store
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 3 {
            let _ = conn.execute(
                "ALTER TABLE datasources ADD COLUMN options_json TEXT NOT NULL DEFAULT '{}'",
                [],
            );
            let _ = conn.execute(
                "ALTER TABLE ingress_routes ADD COLUMN endpoint TEXT NOT NULL DEFAULT ''",
                [],
            );
            let _ = conn.execute(
                "ALTER TABLE ingress_routes ADD COLUMN options_json TEXT NOT NULL DEFAULT '{}'",
                [],
            );
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS ingress_kafka_offsets (
    route_id TEXT NOT NULL,
    partition INTEGER NOT NULL,
    next_offset INTEGER NOT NULL,
    PRIMARY KEY(route_id, partition)
);
INSERT INTO schema_meta(key, value) VALUES('version', '3')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v4: alert enrichment rules (annotation templates + label maps)
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 4 {
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS enrich_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    matchers_json TEXT NOT NULL DEFAULT '{}',
    match_key TEXT NOT NULL DEFAULT '',
    templates_json TEXT NOT NULL DEFAULT '{}',
    mappings_json TEXT NOT NULL DEFAULT '{}',
    write_labels INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
INSERT INTO schema_meta(key, value) VALUES('version', '4')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v5: shared lookup tables + enrich.lookup_table_id
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 5 {
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS lookup_tables (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    key_label TEXT NOT NULL DEFAULT 'instance',
    rows_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#,
            )?;
            let _ = conn.execute(
                "ALTER TABLE enrich_rules ADD COLUMN lookup_table_id TEXT",
                [],
            );
            conn.execute_batch(
                r#"
INSERT INTO schema_meta(key, value) VALUES('version', '5')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v6: enrich rules may reference many lookup tables (M:N)
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 6 {
            let _ = conn.execute(
                "ALTER TABLE enrich_rules ADD COLUMN lookup_table_ids_json TEXT NOT NULL DEFAULT '[]'",
                [],
            );
            // Migrate legacy single lookup_table_id → lookup_table_ids_json array.
            let mut stmt = conn.prepare(
                "SELECT id, lookup_table_id FROM enrich_rules WHERE lookup_table_id IS NOT NULL AND lookup_table_id != ''",
            )?;
            let legacy: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            drop(stmt);
            for (id, tid) in legacy {
                let json = serde_json::to_string(&vec![tid])?;
                conn.execute(
                    "UPDATE enrich_rules SET lookup_table_ids_json=?1 WHERE id=?2",
                    rusqlite::params![json, id],
                )?;
            }
            conn.execute_batch(
                r#"
INSERT INTO schema_meta(key, value) VALUES('version', '6')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v7: enrich field_templates (severity / ip / alertname / summary overrides)
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 7 {
            let _ = conn.execute(
                "ALTER TABLE enrich_rules ADD COLUMN field_templates_json TEXT NOT NULL DEFAULT '{}'",
                [],
            );
            conn.execute_batch(
                r#"
INSERT INTO schema_meta(key, value) VALUES('version', '7')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v8: departments / roles / users (RBAC)
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 8 {
            conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS departments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    permissions_json TEXT NOT NULL DEFAULT '[]',
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL DEFAULT '',
    password_hash TEXT NOT NULL,
    department_id TEXT,
    role_ids_json TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_users_dept ON users(department_id);
CREATE INDEX IF NOT EXISTS idx_departments_parent ON departments(parent_id);

INSERT INTO schema_meta(key, value) VALUES('version', '8')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v9: enrich label_extracts (pre-lookup template extracts)
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 9 {
            let _ = conn.execute(
                "ALTER TABLE enrich_rules ADD COLUMN label_extracts_json TEXT NOT NULL DEFAULT '{}'",
                [],
            );
            conn.execute_batch(
                r#"
INSERT INTO schema_meta(key, value) VALUES('version', '9')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }

        // v10: per-rule lookup match key overrides
        let ver: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key='version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < 10 {
            let _ = conn.execute(
                "ALTER TABLE enrich_rules ADD COLUMN lookup_match_keys_json TEXT NOT NULL DEFAULT '{}'",
                [],
            );
            conn.execute_batch(
                r#"
INSERT INTO schema_meta(key, value) VALUES('version', '10')
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
"#,
            )?;
        }
        Ok(())
    }

    /// Seed admin role + toml bootstrap user when users table is empty.
    pub fn seed_iam_if_empty(&self, username: &str, password: &str) -> Result<()> {
        let count: i64 = {
            let conn = self.lock()?;
            conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .unwrap_or(0)
        };
        if count > 0 {
            return Ok(());
        }

        use crate::iam::{Department, Role, UserAccount};
        use crate::password::hash_password;
        use chrono::Utc;
        use uuid::Uuid;

        let now = Utc::now();
        let dept_id = Uuid::new_v4();
        let role_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        self.upsert_department(&Department {
            id: dept_id,
            name: "默认组织".into(),
            parent_id: None,
            sort_order: 0,
            enabled: true,
            created_at: now,
            updated_at: now,
        })?;

        self.upsert_role(&Role {
            id: role_id,
            name: "admin".into(),
            description: "系统管理员（全部权限）".into(),
            permissions: vec!["*".into()],
            is_system: true,
            created_at: now,
            updated_at: now,
        })?;

        self.upsert_user(&UserAccount {
            id: user_id,
            username: username.to_string(),
            display_name: "管理员".into(),
            password_hash: hash_password(password),
            department_id: Some(dept_id),
            role_ids: vec![role_id],
            enabled: true,
            created_at: now,
            updated_at: now,
        })?;

        tracing::info!("seeded IAM admin user '{username}' with system role admin");
        Ok(())
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("sqlite mutex poisoned"))
    }
}
