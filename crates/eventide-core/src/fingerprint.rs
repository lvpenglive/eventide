//! Stable alert fingerprint from rule id + sorted labels.

use crate::models::Labels;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Compute a hex SHA-256 fingerprint for deduplication.
///
/// Format hashed: `{rule_id}|{k1=v1,k2=v2,...}` with labels sorted by key.
pub fn alert_fingerprint(rule_id: Uuid, labels: &Labels) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule_id.as_bytes());
    hasher.update(b"|");
    let mut first = true;
    for (k, v) in labels {
        if !first {
            hasher.update(b",");
        }
        first = false;
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
    }
    let digest = hasher.finalize();
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn fingerprint_stable_and_order_independent_via_btree() {
        let id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let mut a = BTreeMap::new();
        a.insert("instance".into(), "a".into());
        a.insert("job".into(), "api".into());
        let mut b = BTreeMap::new();
        b.insert("job".into(), "api".into());
        b.insert("instance".into(), "a".into());
        assert_eq!(alert_fingerprint(id, &a), alert_fingerprint(id, &b));
    }

    #[test]
    fn different_labels_different_fingerprint() {
        let id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let mut a = BTreeMap::new();
        a.insert("instance".into(), "a".into());
        let mut b = BTreeMap::new();
        b.insert("instance".into(), "b".into());
        assert_ne!(alert_fingerprint(id, &a), alert_fingerprint(id, &b));
    }
}
