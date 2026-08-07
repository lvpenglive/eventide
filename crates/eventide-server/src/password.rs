//! Password hashing (SHA-256 + salt). Format: `sha256$<salt>$<hex>`.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Console account password policy (create / update / reset / self-change).
/// Seed from `eventide.toml` is not checked so first boot still works.
pub fn validate_password_complexity(password: &str) -> Result<(), String> {
    let n = password.chars().count();
    if n < 8 {
        return Err("密码至少 8 位".into());
    }
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());
    if !has_lower || !has_upper || !has_digit || !has_special {
        return Err(
            "密码须同时包含大写字母、小写字母、数字和特殊字符（如 !@#$%）".into(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PasswordAgeStatus {
    pub enabled: bool,
    pub expired: bool,
    pub warn: bool,
    pub days_left: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
    pub changed_at: DateTime<Utc>,
}

impl PasswordAgeStatus {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": self.enabled,
            "expired": self.expired,
            "warn": self.warn,
            "days_left": self.days_left,
            "expires_at": self.expires_at.map(|t| t.to_rfc3339()),
            "changed_at": self.changed_at.to_rfc3339(),
        })
    }
}

/// Soft expiry reminder status. `max_age_days == 0` disables the feature.
pub fn password_age_status(
    changed_at: DateTime<Utc>,
    max_age_days: u64,
    warn_days: u64,
) -> PasswordAgeStatus {
    if max_age_days == 0 {
        return PasswordAgeStatus {
            enabled: false,
            expired: false,
            warn: false,
            days_left: None,
            expires_at: None,
            changed_at,
        };
    }
    let expires_at = changed_at + Duration::days(max_age_days as i64);
    let now = Utc::now();
    let days_left = (expires_at - now).num_days();
    let expired = now >= expires_at;
    let warn = !expired && days_left <= warn_days as i64;
    PasswordAgeStatus {
        enabled: true,
        expired,
        warn,
        days_left: Some(days_left),
        expires_at: Some(expires_at),
        changed_at,
    }
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

    #[test]
    fn complexity_ok() {
        assert!(validate_password_complexity("Abcd123!").is_ok());
    }

    #[test]
    fn complexity_rejects_weak() {
        assert!(validate_password_complexity("admin123").is_err());
        assert!(validate_password_complexity("Abcdefgh").is_err());
        assert!(validate_password_complexity("Ab1!").is_err());
    }

    #[test]
    fn age_disabled() {
        let s = password_age_status(Utc::now(), 0, 14);
        assert!(!s.enabled);
        assert!(!s.expired);
    }

    #[test]
    fn age_warn_and_expire() {
        let old = Utc::now() - Duration::days(80);
        let s = password_age_status(old, 90, 14);
        assert!(s.enabled);
        assert!(s.warn);
        assert!(!s.expired);
        assert!(s.days_left.unwrap() <= 14);

        let older = Utc::now() - Duration::days(100);
        let e = password_age_status(older, 90, 14);
        assert!(e.expired);
    }
}
