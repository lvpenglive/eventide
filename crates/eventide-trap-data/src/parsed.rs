//! Intermediate trap fields used by policy matching (filled by SNMP parse in eventide-trap).

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ParsedTrap {
    pub version: String,
    pub community: String,
    pub peer_ip: String,
    #[allow(dead_code)]
    pub peer_port: u16,
    pub trap_oid: String,
    pub alertname: String,
    pub severity_hint: Option<String>,
    pub varbinds: BTreeMap<String, String>,
    pub raw_note: String,
}
