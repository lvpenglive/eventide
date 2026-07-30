//! SNMPv1 / SNMPv2c Trap → intermediate fields.

use snmp_parser::snmp::{
    NetworkAddress, ObjectSyntax, PduType, SnmpMessage, SnmpPdu, TrapType, VarBindValue,
};
use snmp_parser::{parse_snmp_v1, parse_snmp_v2c, Oid};
use std::collections::BTreeMap;
use std::net::SocketAddr;

pub use eventide_trap_data::ParsedTrap;

/// Well-known snmpTrapOID.0
const SNMP_TRAP_OID: &str = "1.3.6.1.6.3.1.1.4.1.0";
/// sysUpTime.0
const SYS_UPTIME_OID: &str = "1.3.6.1.2.1.1.3.0";

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a trap PDU (type={0:?})")]
    NotTrap(PduType),
    #[error("snmp parse: {0}")]
    Snmp(String),
    #[error("community mismatch")]
    Community,
}

pub fn parse_udp_datagram(
    buf: &[u8],
    peer: SocketAddr,
    expected_community: &str,
) -> Result<ParsedTrap, ParseError> {
    // Try v2c first (common), then v1.
    if let Ok((_, msg)) = parse_snmp_v2c(buf) {
        return from_message(msg, peer, expected_community, "v2c");
    }
    if let Ok((_, msg)) = parse_snmp_v1(buf) {
        return from_message(msg, peer, expected_community, "v1");
    }
    Err(ParseError::Snmp("neither SNMPv1 nor v2c".into()))
}

fn from_message(
    msg: SnmpMessage<'_>,
    peer: SocketAddr,
    expected_community: &str,
    version_label: &str,
) -> Result<ParsedTrap, ParseError> {
    if !expected_community.is_empty() && msg.community != expected_community {
        return Err(ParseError::Community);
    }

    let peer_ip = peer.ip().to_string();
    let peer_port = peer.port();

    match msg.pdu {
        SnmpPdu::TrapV1(trap) => {
            let mut varbinds = BTreeMap::new();
            for v in trap.vars_iter() {
                varbinds.insert(oid_str(&v.oid), varbind_value(&v.val));
            }
            let trap_oid = format_v1_trap_oid(&trap.enterprise, trap.generic_trap, trap.specific_trap);
            let alertname = v1_alertname(trap.generic_trap, trap.specific_trap, &trap_oid);
            let severity_hint = v1_severity(trap.generic_trap);
            let agent = match trap.agent_addr {
                NetworkAddress::IPv4(a) => a.to_string(),
            };
            Ok(ParsedTrap {
                version: version_label.into(),
                community: msg.community,
                peer_ip: agent,
                peer_port,
                trap_oid: trap_oid.clone(),
                alertname,
                severity_hint,
                varbinds,
                raw_note: format!("v1 enterprise={}", oid_str(&trap.enterprise)),
            })
        }
        SnmpPdu::Generic(pdu) if pdu.pdu_type == PduType::TrapV2 || pdu.pdu_type == PduType::InformRequest => {
            let mut varbinds = BTreeMap::new();
            for v in &pdu.var {
                varbinds.insert(oid_str(&v.oid), varbind_value(&v.val));
            }
            let trap_oid = varbinds
                .get(SNMP_TRAP_OID)
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown".into());
            let alertname = trap_oid_leaf(&trap_oid);
            let _uptime = varbinds.get(SYS_UPTIME_OID).cloned();
            Ok(ParsedTrap {
                version: version_label.into(),
                community: msg.community,
                peer_ip,
                peer_port,
                trap_oid: trap_oid.clone(),
                alertname,
                severity_hint: None,
                varbinds,
                raw_note: format!("pdu={:?}", pdu.pdu_type),
            })
        }
        other => Err(ParseError::NotTrap(other.pdu_type())),
    }
}

fn oid_str(oid: &Oid<'_>) -> String {
    // asn1_rs::Oid Display → dotted decimal
    format!("{oid}")
}

fn format_v1_trap_oid(enterprise: &Oid<'_>, generic: TrapType, specific: u32) -> String {
    let ent = oid_str(enterprise);
    if generic.0 == TrapType::ENTERPRISE_SPECIFIC.0 {
        format!("{ent}.0.{specific}")
    } else {
        // RFC-style: enterprise + generic mapping under snmpTraps
        format!("1.3.6.1.6.3.1.1.5.{}", generic.0 + 1)
    }
}

fn v1_alertname(generic: TrapType, specific: u32, trap_oid: &str) -> String {
    match generic.0 {
        0 => "coldStart".into(),
        1 => "warmStart".into(),
        2 => "linkDown".into(),
        3 => "linkUp".into(),
        4 => "authenticationFailure".into(),
        5 => "egpNeighborLoss".into(),
        6 => format!("enterpriseSpecific-{specific}"),
        _ => trap_oid_leaf(trap_oid),
    }
}

fn v1_severity(generic: TrapType) -> Option<String> {
    match generic.0 {
        2 | 4 => Some("disaster".into()),
        0 | 1 | 3 => Some("information".into()),
        _ => None,
    }
}

fn trap_oid_leaf(trap_oid: &str) -> String {
    trap_oid
        .rsplit('.')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| format!("snmpTrap-{s}"))
        .unwrap_or_else(|| "snmpTrap".into())
}

fn varbind_value(val: &VarBindValue<'_>) -> String {
    match val {
        VarBindValue::Value(os) => object_syntax_str(os),
        VarBindValue::Unspecified => "unspecified".into(),
        VarBindValue::NoSuchObject => "noSuchObject".into(),
        VarBindValue::NoSuchInstance => "noSuchInstance".into(),
        VarBindValue::EndOfMibView => "endOfMibView".into(),
    }
}

fn object_syntax_str(os: &ObjectSyntax<'_>) -> String {
    match os {
        ObjectSyntax::Number(n) => n.to_string(),
        ObjectSyntax::String(b) => String::from_utf8_lossy(b).into_owned(),
        ObjectSyntax::Object(oid) => oid_str(oid),
        ObjectSyntax::BitString(bs) => format!("bits:{bs:?}"),
        ObjectSyntax::Empty => String::new(),
        ObjectSyntax::UnknownSimple(a) => format!("raw:{}", hex_preview(a.data)),
        ObjectSyntax::IpAddress(NetworkAddress::IPv4(a)) => a.to_string(),
        ObjectSyntax::Counter32(c) => c.to_string(),
        ObjectSyntax::Gauge32(g) => g.to_string(),
        ObjectSyntax::TimeTicks(t) => t.to_string(),
        ObjectSyntax::Opaque(b) => format!("opaque:{}", hex_preview(b)),
        ObjectSyntax::NsapAddress(b) => format!("nsap:{}", hex_preview(b)),
        ObjectSyntax::Counter64(n) => n.to_string(),
        ObjectSyntax::UInteger32(n) => n.to_string(),
        ObjectSyntax::UnknownApplication(a) => format!("app:{}", hex_preview(a.data)),
    }
}

fn hex_preview(b: &[u8]) -> String {
    let n = b.len().min(32);
    let mut s = String::with_capacity(n * 2);
    for x in &b[..n] {
        s.push_str(&format!("{x:02x}"));
    }
    if b.len() > n {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_name() {
        assert_eq!(trap_oid_leaf("1.3.6.1.4.1.9.9.41.2.0.1"), "snmpTrap-1");
    }
}
