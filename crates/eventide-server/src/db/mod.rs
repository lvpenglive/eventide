//! MySQL persistence for Eventide.

mod iam_repo;
mod repo;

use anyhow::{bail, Context, Result};
use mysql::prelude::*;
use mysql::{Opts, OptsBuilder, Pool, PooledConn};

pub struct Db {
    pool: Pool,
}

impl Db {
    pub fn connect(url: &str) -> Result<Self> {
        ensure_database(url)?;
        let opts = Opts::from_url(url).context("parse mysql_url")?;
        let pool = Pool::new(opts).context("mysql pool")?;
        Ok(Self { pool })
    }

    pub fn conn(&self) -> Result<PooledConn> {
        self.pool.get_conn().context("mysql get_conn")
    }

    /// Clone of the underlying MySQL pool (for shared trap stores).
    pub fn pool(&self) -> Pool {
        self.pool.clone()
    }

    pub fn migrate(&self) -> Result<()> {
        let mut conn = self.conn()?;
        // MySQL drivers often only run the first statement in a batch — execute one-by-one.
        const STMTS: &[&str] = &[
            r#"CREATE TABLE IF NOT EXISTS schema_meta (
    `key` VARCHAR(64) PRIMARY KEY,
    `value` VARCHAR(255) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS datasources (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(64) NOT NULL,
    url TEXT NOT NULL,
    options_json MEDIUMTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS notify_channels (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(64) NOT NULL,
    url TEXT NOT NULL,
    secret TEXT NULL,
    options_json MEDIUMTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS rules (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    datasource_id CHAR(36) NOT NULL,
    expr TEXT NOT NULL,
    comparator VARCHAR(16) NOT NULL,
    threshold DOUBLE NOT NULL,
    for_seconds INT NOT NULL DEFAULT 0,
    interval_seconds INT NOT NULL DEFAULT 30,
    severity VARCHAR(32) NOT NULL,
    labels_json MEDIUMTEXT NOT NULL,
    annotations_json MEDIUMTEXT NOT NULL,
    channel_ids_json MEDIUMTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    last_run_at VARCHAR(64) NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            // fingerprint: Ingress prefixes route UUID + Trap uses `ip|oid|varbinds`
            // (often >128). Keep ≤768 so utf8mb4 unique index stays within InnoDB limit.
            r#"CREATE TABLE IF NOT EXISTS alert_events (
    id CHAR(36) PRIMARY KEY,
    rule_id CHAR(36) NOT NULL,
    fingerprint VARCHAR(768) NOT NULL,
    status VARCHAR(32) NOT NULL,
    severity VARCHAR(32) NOT NULL,
    labels_json MEDIUMTEXT NOT NULL,
    annotations_json MEDIUMTEXT NOT NULL,
    value DOUBLE NULL,
    starts_at VARCHAR(64) NOT NULL,
    ends_at VARCHAR(64) NULL,
    pending_since VARCHAR(64) NULL,
    last_evaluated_at VARCHAR(64) NOT NULL,
    notified_firing TINYINT NOT NULL DEFAULT 0,
    notified_resolved TINYINT NOT NULL DEFAULT 0,
    UNIQUE KEY uk_alert_fp (fingerprint),
    KEY idx_alert_status (status),
    KEY idx_alert_rule (rule_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS silences (
    id CHAR(36) PRIMARY KEY,
    rule_id CHAR(36) NULL,
    matchers_json MEDIUMTEXT NOT NULL,
    starts_at VARCHAR(64) NOT NULL,
    ends_at VARCHAR(64) NOT NULL,
    comment TEXT NOT NULL,
    created_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS notify_logs (
    id CHAR(36) PRIMARY KEY,
    alert_id CHAR(36) NOT NULL,
    channel_id CHAR(36) NOT NULL,
    transition VARCHAR(32) NOT NULL,
    success TINYINT NOT NULL,
    error TEXT NULL,
    body MEDIUMTEXT NOT NULL,
    created_at VARCHAR(64) NOT NULL,
    KEY idx_notify_logs_created (created_at),
    KEY idx_notify_logs_channel (channel_id),
    KEY idx_notify_logs_alert (alert_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS ingress_routes (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(64) NOT NULL,
    token TEXT NULL,
    endpoint TEXT NOT NULL,
    options_json MEDIUMTEXT NOT NULL,
    channel_ids_json MEDIUMTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS ingress_kafka_offsets (
    route_id CHAR(36) NOT NULL,
    `partition` INT NOT NULL,
    next_offset BIGINT NOT NULL,
    PRIMARY KEY (route_id, `partition`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS enrich_rules (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    kind VARCHAR(64) NOT NULL,
    matchers_json MEDIUMTEXT NOT NULL,
    match_key VARCHAR(255) NOT NULL DEFAULT '',
    templates_json MEDIUMTEXT NOT NULL,
    mappings_json MEDIUMTEXT NOT NULL,
    write_labels TINYINT NOT NULL DEFAULT 0,
    enabled TINYINT NOT NULL DEFAULT 1,
    priority INT NOT NULL DEFAULT 100,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    lookup_table_id CHAR(36) NULL,
    lookup_table_ids_json MEDIUMTEXT NOT NULL,
    field_templates_json MEDIUMTEXT NOT NULL,
    label_extracts_json MEDIUMTEXT NOT NULL,
    lookup_match_keys_json MEDIUMTEXT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS lookup_tables (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    key_label VARCHAR(128) NOT NULL DEFAULT 'instance',
    rows_json LONGTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS departments (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    parent_id CHAR(36) NULL,
    sort_order INT NOT NULL DEFAULT 0,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    KEY idx_departments_parent (parent_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS roles (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    permissions_json MEDIUMTEXT NOT NULL,
    is_system TINYINT NOT NULL DEFAULT 0,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    UNIQUE KEY uk_roles_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS users (
    id CHAR(36) PRIMARY KEY,
    username VARCHAR(128) NOT NULL,
    display_name VARCHAR(255) NOT NULL DEFAULT '',
    password_hash TEXT NOT NULL,
    department_id CHAR(36) NULL,
    role_ids_json MEDIUMTEXT NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    created_at VARCHAR(64) NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    UNIQUE KEY uk_users_username (username),
    KEY idx_users_dept (department_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS app_kv (
    `key` VARCHAR(64) PRIMARY KEY,
    `value` MEDIUMTEXT NOT NULL,
    updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS trap_policies (
    id CHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    trap_oid VARCHAR(512) NOT NULL,
    match_mode VARCHAR(16) NOT NULL DEFAULT 'exact',
    severity VARCHAR(32) NOT NULL,
    enabled TINYINT NOT NULL DEFAULT 1,
    summary_template MEDIUMTEXT NOT NULL,
    description MEDIUMTEXT NOT NULL,
    objects_json MEDIUMTEXT NOT NULL,
    object_oids_json MEDIUMTEXT NOT NULL,
    keywords_json MEDIUMTEXT NOT NULL,
    module VARCHAR(255) NOT NULL DEFAULT '',
    status VARCHAR(32) NOT NULL DEFAULT '',
    resolve_oid VARCHAR(512) NOT NULL DEFAULT '',
    resolve_values_json MEDIUMTEXT NOT NULL,
    fingerprint_oids_json MEDIUMTEXT NOT NULL,
    severity_oid VARCHAR(512) NOT NULL DEFAULT '',
    severity_map_json MEDIUMTEXT NOT NULL,
    updated_at VARCHAR(64) NOT NULL,
    KEY idx_trap_policies_oid (trap_oid(191)),
    KEY idx_trap_policies_updated (updated_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            r#"CREATE TABLE IF NOT EXISTS trap_mibs (
    id VARCHAR(255) PRIMARY KEY,
    filename VARCHAR(512) NOT NULL,
    object_key VARCHAR(1024) NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    uploaded_at VARCHAR(64) NOT NULL,
    parse_ok TINYINT NOT NULL DEFAULT 0,
    error MEDIUMTEXT NULL,
    notification_count INT NOT NULL DEFAULT 0,
    node_count INT NOT NULL DEFAULT 0,
    module_oid VARCHAR(512) NULL,
    updated_at VARCHAR(64) NOT NULL,
    KEY idx_trap_mibs_updated (updated_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
        ];
        for sql in STMTS {
            conn.query_drop(*sql)
                .with_context(|| format!("migrate: {sql}"))?;
        }
        // v15: widen fingerprint — Trap/Kafka ingress fingerprints exceed VARCHAR(128).
        conn.query_drop(
            "ALTER TABLE alert_events MODIFY fingerprint VARCHAR(768) NOT NULL",
        )
        .with_context(|| "migrate: widen alert_events.fingerprint")?;
        conn.exec_drop(
            r#"INSERT INTO schema_meta (`key`, `value`) VALUES ('version', '15')
               ON DUPLICATE KEY UPDATE `value`=VALUES(`value`)"#,
            (),
        )?;
        Ok(())
    }

    /// Seed admin role + toml bootstrap user when users table is empty.
    pub fn seed_iam_if_empty(&self, username: &str, password: &str) -> Result<()> {
        let count: i64 = {
            let mut conn = self.conn()?;
            conn.exec_first("SELECT COUNT(*) FROM users", ())?
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
}

/// Create the database named in `mysql_url` if it does not exist yet.
fn ensure_database(url: &str) -> Result<()> {
    let opts = Opts::from_url(url).context("parse mysql_url")?;
    let Some(db_name) = opts.get_db_name().map(|s| s.to_owned()) else {
        return Ok(());
    };
    // Reject odd names to avoid injection in CREATE DATABASE.
    if !db_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid mysql database name: {db_name}");
    }
    let bootstrap = OptsBuilder::from_opts(opts.clone()).db_name(None::<String>);
    let pool = Pool::new(bootstrap).context("mysql bootstrap pool")?;
    let mut conn = pool.get_conn().context("mysql bootstrap conn")?;
    conn.query_drop(format!(
        "CREATE DATABASE IF NOT EXISTS `{db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .with_context(|| format!("create database {db_name}"))?;
    Ok(())
}
