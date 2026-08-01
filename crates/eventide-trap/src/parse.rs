//! SNMPv1 / SNMPv2c / SNMPv3 (USM) Trap → intermediate fields.

use crate::config::SnmpV3User;
use crate::usm::{
    decrypt_scoped_pdu, localize_key, localize_priv_key, resolve_user, verify_authentication,
    AuthProtocol, PrivProtocol, UsmError,
};
use snmp_parser::asn1_rs::{Any, FromBer, Sequence};
use snmp_parser::snmp::{
    NetworkAddress, ObjectSyntax, PduType, SnmpMessage, SnmpPdu, TrapType, VarBindValue,
};
use snmp_parser::{
    parse_snmp_v1, parse_snmp_v2c, parse_snmp_v3, ScopedPduData, SecurityParameters, Oid,
};
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
    #[error("snmpv3 usm: {0}")]
    Usm(#[from] UsmError),
    #[error("snmpv3 not configured (empty snmpv3_users)")]
    V3Disabled,
}

pub fn parse_udp_datagram(
    buf: &[u8],
    peer: SocketAddr,
    expected_community: &str,
    snmpv3_users: &[SnmpV3User],
) -> Result<ParsedTrap, ParseError> {
    // Try v2c first (common), then v1, then v3.
    if let Ok((_, msg)) = parse_snmp_v2c(buf) {
        return from_message(msg, peer, expected_community, "v2c");
    }
    if let Ok((_, msg)) = parse_snmp_v1(buf) {
        return from_message(msg, peer, expected_community, "v1");
    }
    if let Ok((_, msg)) = parse_snmp_v3(buf) {
        return from_v3(buf, msg, peer, snmpv3_users);
    }
    Err(ParseError::Snmp("neither SNMPv1, v2c, nor v3".into()))
}

fn from_v3(
    whole_msg: &[u8],
    msg: snmp_parser::SnmpV3Message<'_>,
    peer: SocketAddr,
    users: &[SnmpV3User],
) -> Result<ParsedTrap, ParseError> {
    if users.is_empty() {
        return Err(ParseError::V3Disabled);
    }
    let SecurityParameters::USM(usm) = &msg.security_params else {
        return Err(ParseError::Snmp("snmpv3 security model is not USM".into()));
    };
    let resolved = resolve_user(users, &usm.msg_user_name, usm.msg_authoritative_engine_id)?;

    if msg.header_data.is_authenticated() || resolved.auth != AuthProtocol::None {
        if resolved.auth == AuthProtocol::None {
            return Err(UsmError::AuthFailed.into());
        }
        let auth_key = localize_key(
            resolved.auth,
            &resolved.user.auth_password,
            usm.msg_authoritative_engine_id,
        );
        verify_authentication(
            whole_msg,
            usm.msg_authentication_parameters,
            &auth_key,
            resolved.auth,
        )?;
    }

    tracing::debug!(
        boots = usm.msg_authoritative_engine_boots,
        time = usm.msg_authoritative_engine_time,
        user = %usm.msg_user_name,
        "snmpv3 usm (time window not enforced for traps)"
    );

    match &msg.data {
        ScopedPduData::Plaintext(scoped) => {
            if msg.header_data.is_encrypted() {
                return Err(ParseError::Snmp(
                    "encrypted flag but plaintext scoped PDU".into(),
                ));
            }
            from_pdu(
                &scoped.data,
                peer,
                "v3",
                &usm.msg_user_name,
                format!(
                    "v3 user={} engine={}",
                    usm.msg_user_name,
                    hex_preview(usm.msg_authoritative_engine_id)
                ),
            )
        }
        ScopedPduData::Encrypted(cipher) => {
            if resolved.privy == PrivProtocol::None {
                return Err(
                    UsmError::PrivFailed("encrypted PDU but user has no priv".into()).into(),
                );
            }
            let priv_key = localize_priv_key(
                resolved.auth,
                &resolved.user.priv_password,
                usm.msg_authoritative_engine_id,
                resolved.privy,
            );
            let plain = decrypt_scoped_pdu(
                cipher,
                &priv_key,
                resolved.privy,
                usm.msg_authoritative_engine_boots,
                usm.msg_authoritative_engine_time,
                usm.msg_privacy_parameters,
            )?;
            from_scoped_plaintext(
                &plain,
                peer,
                &usm.msg_user_name,
                usm.msg_authoritative_engine_id,
            )
        }
    }
}

fn from_scoped_plaintext(
    scoped_bytes: &[u8],
    peer: SocketAddr,
    username: &str,
    engine_id: &[u8],
) -> Result<ParsedTrap, ParseError> {
    let pdu_raw = extract_pdu_bytes(scoped_bytes)?;
    let wrapped = wrap_as_v2c(&pdu_raw);
    let (_, msg) = parse_snmp_v2c(&wrapped)
        .map_err(|e| ParseError::Snmp(format!("scoped PDU after decrypt: {e:?}")))?;
    from_pdu(
        &msg.pdu,
        peer,
        "v3",
        username,
        format!("v3 user={username} engine={}", hex_preview(engine_id)),
    )
}

fn extract_pdu_bytes(scoped: &[u8]) -> Result<Vec<u8>, ParseError> {
    // ScopedPDU ::= SEQUENCE { contextEngineID, contextName, data }
    // Tolerate trailing DES padding after the SEQUENCE.
    let (_rest, seq) = Sequence::from_ber(scoped)
        .map_err(|e| ParseError::Snmp(format!("scoped SEQUENCE: {e:?}")))?;
    let content: &[u8] = seq.content.as_ref();
    let (i, _eng) = <&[u8]>::from_ber(content)
        .map_err(|e| ParseError::Snmp(format!("contextEngineID: {e:?}")))?;
    let (i, _name) =
        <&[u8]>::from_ber(i).map_err(|e| ParseError::Snmp(format!("contextName: {e:?}")))?;
    let (rest, _any) =
        Any::from_ber(i).map_err(|e| ParseError::Snmp(format!("scoped PDU ANY: {e:?}")))?;
    let len = i.len() - rest.len();
    Ok(i[..len].to_vec())
}

fn wrap_as_v2c(pdu_raw: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + pdu_raw.len());
    // INTEGER version = 1 (v2c)
    body.extend_from_slice(&[0x02, 0x01, 0x01]);
    // OCTET STRING community "v3"
    body.extend_from_slice(&[0x04, 0x02, b'v', b'3']);
    body.extend_from_slice(pdu_raw);
    encode_ber_sequence(&body)
}

fn encode_ber_sequence(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + content.len());
    out.push(0x30);
    encode_ber_len(&mut out, content.len());
    out.extend_from_slice(content);
    out
}

fn encode_ber_len(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
    } else if len <= 0xff {
        out.push(0x81);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push(0x82);
        out.push((len >> 8) as u8);
        out.push((len & 0xff) as u8);
    } else {
        out.push(0x83);
        out.push((len >> 16) as u8);
        out.push(((len >> 8) & 0xff) as u8);
        out.push((len & 0xff) as u8);
    }
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
    from_pdu(
        &msg.pdu,
        peer,
        version_label,
        &msg.community,
        format!("{version_label} community={}", msg.community),
    )
}

fn from_pdu(
    pdu: &SnmpPdu<'_>,
    peer: SocketAddr,
    version_label: &str,
    community_or_user: &str,
    raw_note: String,
) -> Result<ParsedTrap, ParseError> {
    let peer_ip = peer.ip().to_string();
    let peer_port = peer.port();

    match pdu {
        SnmpPdu::TrapV1(trap) => {
            let mut varbinds = BTreeMap::new();
            for v in trap.vars_iter() {
                varbinds.insert(oid_str(&v.oid), varbind_value(&v.val));
            }
            let trap_oid =
                format_v1_trap_oid(&trap.enterprise, trap.generic_trap, trap.specific_trap);
            let alertname = v1_alertname(trap.generic_trap, trap.specific_trap, &trap_oid);
            let severity_hint = v1_severity(trap.generic_trap);
            let agent = match trap.agent_addr {
                NetworkAddress::IPv4(a) => a.to_string(),
            };
            Ok(ParsedTrap {
                version: version_label.into(),
                community: community_or_user.into(),
                peer_ip: agent,
                peer_port,
                trap_oid: trap_oid.clone(),
                alertname,
                severity_hint,
                varbinds,
                raw_note: format!("{raw_note}; v1 enterprise={}", oid_str(&trap.enterprise)),
            })
        }
        SnmpPdu::Generic(pdu)
            if pdu.pdu_type == PduType::TrapV2 || pdu.pdu_type == PduType::InformRequest =>
        {
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
                community: community_or_user.into(),
                peer_ip,
                peer_port,
                trap_oid: trap_oid.clone(),
                alertname,
                severity_hint: None,
                varbinds,
                raw_note: format!("{raw_note}; pdu={:?}", pdu.pdu_type),
            })
        }
        other => Err(ParseError::NotTrap(other.pdu_type())),
    }
}

fn oid_str(oid: &Oid<'_>) -> String {
    format!("{oid}")
}

fn format_v1_trap_oid(enterprise: &Oid<'_>, generic: TrapType, specific: u32) -> String {
    let ent = oid_str(enterprise);
    if generic.0 == TrapType::ENTERPRISE_SPECIFIC.0 {
        format!("{ent}.0.{specific}")
    } else {
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
