//! Shared Trap MIB / policy storage (MySQL + RustFS) for Eventide server & trap.

mod db;
mod mib_store;
mod parsed;
mod policy_redis;
mod policy_store;
mod policy_xlsx;
mod s3_store;

pub use db::{
    connect as connect_mysql, ensure_trap_schema, row_i32, row_i64, row_i8, row_string,
    row_string_opt, DDL_TRAP_MIBS, DDL_TRAP_POLICIES,
};
pub use mib_store::{
    MibEntry, MibStore, NodeDetail, PolicyExport, TrapPolicyDraft, TreeNode,
};
pub use parsed::ParsedTrap;
pub use policy_redis::{
    PolicyRedis, PolicySnapshot, POLICY_CHANGED_CHANNEL, POLICY_SNAPSHOT_KEY,
};
pub use policy_store::{
    build_fingerprint, build_template_vars, normalize_event_status, render_template,
    resolve_event_status, resolve_object_name, resolve_severity, ImportMode, ImportResult,
    PolicyStore, TrapPolicy,
};
pub use policy_xlsx::{read_xlsx_drafts, write_xlsx_from_drafts, write_xlsx_from_policies};
pub use s3_store::{object_key, S3Store};
