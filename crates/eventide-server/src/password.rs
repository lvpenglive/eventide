//! Password hashing (SHA-256 + salt). Format: `sha256$<salt>$<hex>`.

use sha2::{Digest, Sha256};
use uuid::Uuid;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_password(password: &str) -> String {
    let salt = Uuid::new_v4().to_string();
    let digest = Sha256::digest(format!("{salt}{password}").as_bytes());
    format!("sha256${salt}${}", to_hex(&digest))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let mut parts = stored.splitn(3, '$');
    let Some(algo) = parts.next() else {
        return false;
    };
    let Some(salt) = parts.next() else {
        return false;
    };
    let Some(expect) = parts.next() else {
        return false;
    };
    if algo != "sha256" {
        return false;
    }
    let digest = Sha256::digest(format!("{salt}{password}").as_bytes());
    to_hex(&digest) == expect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let h = hash_password("admin123");
        assert!(verify_password("admin123", &h));
        assert!(!verify_password("wrong", &h));
    }
}
