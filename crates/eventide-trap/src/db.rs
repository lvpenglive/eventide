//! MySQL pool + Trap schema (shared with eventide-server migrate v14).

use anyhow::{bail, Context, Result};
use mysql::prelude::*;
use mysql::{Opts, OptsBuilder, Pool, PooledConn};

pub const DDL_TRAP_POLICIES: &str = r#"CREATE TABLE IF NOT EXISTS trap_policies (
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
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#;

pub const DDL_TRAP_MIBS: &str = r#"CREATE TABLE IF NOT EXISTS trap_mibs (
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
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#;

pub fn connect(mysql_url: &str) -> Result<Pool> {
    if mysql_url.trim().is_empty() {
        bail!("mysql_url is required for trap policies / MIB metadata");
    }
    ensure_database(mysql_url)?;
    let opts = Opts::from_url(mysql_url).context("parse mysql_url")?;
    Pool::new(opts).context("mysql pool")
}

pub fn ensure_trap_schema(pool: &Pool) -> Result<()> {
    let mut conn = pool.get_conn().context("mysql get_conn")?;
    conn.query_drop(DDL_TRAP_POLICIES)
        .context("create trap_policies")?;
    conn.query_drop(DDL_TRAP_MIBS)
        .context("create trap_mibs")?;
    Ok(())
}

pub fn conn(pool: &Pool) -> Result<PooledConn> {
    pool.get_conn().context("mysql get_conn")
}

fn ensure_database(url: &str) -> Result<()> {
    let opts = Opts::from_url(url).context("parse mysql_url")?;
    let Some(db_name) = opts.get_db_name().map(|s| s.to_owned()) else {
        return Ok(());
    };
    if !db_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid mysql database name: {db_name}");
    }
    let bootstrap = OptsBuilder::from_opts(opts.clone()).db_name(None::<String>);
    let pool = Pool::new(bootstrap).context("mysql bootstrap pool")?;
    let mut c = pool.get_conn().context("mysql bootstrap conn")?;
    c.query_drop(format!(
        "CREATE DATABASE IF NOT EXISTS `{db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .with_context(|| format!("create database {db_name}"))?;
    Ok(())
}
