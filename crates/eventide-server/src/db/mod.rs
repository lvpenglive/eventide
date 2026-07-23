//! SQLite persistence for Eventide.

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
        Ok(())
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("sqlite mutex poisoned"))
    }
}
