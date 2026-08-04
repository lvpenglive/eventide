//! Offline product license: Ed25519-signed JWT.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Canonical issuer claim.
pub const ISSUER: &str = "eventide";

/// Embedded vendor public key (PKCS8 / SubjectPublicKeyInfo PEM).
/// Must match the private key used by `eventide-license` issue tool.
pub const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
MCowBQYDK2VwAyEA8J7GKlMhZGq4DxA1oUy2ZeIX7gExFMDghAMkQnJlfBE=\n\
-----END PUBLIC KEY-----\n";

/// Trial length from first boot without a commercial license.
pub const TRIAL_DAYS: i64 = 30;

pub const REQUEST_FORMAT: &str = "eventide-license-request";
pub const LICENSE_FILE_FORMAT: &str = "eventide-license";

/// Customer-exported authorization request (send to vendor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRequest {
    pub format: String,
    pub version: u32,
    pub product: String,
    pub install_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    pub requested_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl LicenseRequest {
    pub fn new(install_id: impl Into<String>, customer: Option<String>) -> Self {
        Self {
            format: REQUEST_FORMAT.into(),
            version: 1,
            product: "eventide".into(),
            install_id: install_id.into(),
            customer: customer
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            requested_at: Utc::now().to_rfc3339(),
            note: None,
        }
    }

    pub fn parse_json(s: &str) -> anyhow::Result<Self> {
        let req: Self = serde_json::from_str(s.trim())?;
        if req.format != REQUEST_FORMAT {
            anyhow::bail!("不是 Eventide 授权申请文件（format={}）", req.format);
        }
        if req.install_id.trim().is_empty() {
            anyhow::bail!("申请文件缺少 install_id");
        }
        Ok(req)
    }
}

/// Vendor-issued license file (customer imports this).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFile {
    pub format: String,
    pub version: u32,
    pub product: String,
    /// Ed25519 JWT
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

impl LicenseFile {
    pub fn from_claims(token: String, claims: &LicenseClaims) -> Self {
        Self {
            format: LICENSE_FILE_FORMAT.into(),
            version: 1,
            product: "eventide".into(),
            token,
            customer: Some(claims.sub.clone()),
            install_id: claims.install_id.clone(),
            edition: Some(claims.edition.clone()),
            issued_at: Some(claims.issued_at().to_rfc3339()),
            expires_at: Some(claims.expires_at().to_rfc3339()),
        }
    }

    /// Accept license file JSON or raw JWT string.
    pub fn parse_import(s: &str) -> anyhow::Result<Self> {
        let t = s.trim();
        if t.is_empty() {
            anyhow::bail!("授权内容为空");
        }
        if t.starts_with('{') {
            let file: Self = serde_json::from_str(t)?;
            if file.format != LICENSE_FILE_FORMAT {
                anyhow::bail!("不是 Eventide 授权文件（format={}）", file.format);
            }
            if file.token.trim().is_empty() {
                anyhow::bail!("授权文件缺少 token");
            }
            return Ok(file);
        }
        // Raw JWT
        Ok(Self {
            format: LICENSE_FILE_FORMAT.into(),
            version: 1,
            product: "eventide".into(),
            token: t.to_string(),
            customer: None,
            install_id: None,
            edition: None,
            issued_at: None,
            expires_at: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseClaims {
    pub iss: String,
    /// Customer / organization name.
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
    #[serde(default = "default_edition")]
    pub edition: String,
    /// When set, must match the installation's `install_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_id: Option<String>,
}

fn default_edition() -> String {
    "standard".into()
}

impl LicenseClaims {
    pub fn expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.exp, 0).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.iat, 0).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
    }
}

#[derive(Debug, Clone)]
pub enum VerifyError {
    Jwt(String),
    Issuer,
    Expired,
    InstallMismatch { expected: String, got: String },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jwt(e) => write!(f, "许可证签名无效：{e}"),
            Self::Issuer => write!(f, "许可证签发方无效"),
            Self::Expired => write!(f, "许可证已过期"),
            Self::InstallMismatch { expected, got } => {
                write!(f, "许可证与本机安装不匹配（期望 {expected}，证内 {got}）")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// Verify JWT with the embedded public key. Optionally enforce `install_id`.
pub fn verify_license(
    token: &str,
    install_id: Option<&str>,
    require_not_expired: bool,
) -> Result<LicenseClaims, VerifyError> {
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.set_issuer(&[ISSUER]);
    validation.validate_exp = require_not_expired;
    // `sub` is customer name, not a fixed subject list.
    validation.set_required_spec_claims(&["exp", "iat", "iss", "sub"]);

    let data = decode::<LicenseClaims>(
        token.trim(),
        &DecodingKey::from_ed_pem(PUBLIC_KEY_PEM.as_bytes())
            .map_err(|e| VerifyError::Jwt(e.to_string()))?,
        &validation,
    )
    .map_err(|e| {
        let msg = e.to_string();
        if msg.to_ascii_lowercase().contains("expired") {
            VerifyError::Expired
        } else {
            VerifyError::Jwt(msg)
        }
    })?;

    let claims = data.claims;
    if claims.iss != ISSUER {
        return Err(VerifyError::Issuer);
    }
    if require_not_expired && claims.exp <= Utc::now().timestamp() {
        return Err(VerifyError::Expired);
    }
    if let Some(got) = claims.install_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        match install_id.map(str::trim).filter(|s| !s.is_empty()) {
            Some(want) if want == got => {}
            Some(want) => {
                return Err(VerifyError::InstallMismatch {
                    expected: want.to_string(),
                    got: got.to_string(),
                });
            }
            None => {
                return Err(VerifyError::InstallMismatch {
                    expected: "(missing)".into(),
                    got: got.to_string(),
                });
            }
        }
    }
    Ok(claims)
}

/// Issue a signed license JWT with the given private key PEM.
pub fn issue_license(
    private_pem: &[u8],
    customer: &str,
    days: i64,
    edition: &str,
    install_id: Option<&str>,
) -> anyhow::Result<String> {
    let now = Utc::now();
    let exp = now + Duration::days(days.max(1));
    let claims = LicenseClaims {
        iss: ISSUER.into(),
        sub: customer.trim().to_string(),
        iat: now.timestamp(),
        exp: exp.timestamp(),
        edition: if edition.trim().is_empty() {
            "standard".into()
        } else {
            edition.trim().into()
        },
        install_id: install_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    };
    let mut header = Header::new(Algorithm::EdDSA);
    header.typ = Some("JWT".into());
    let key = EncodingKey::from_ed_pem(private_pem)?;
    Ok(encode(&header, &claims, &key)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matching pair used only in unit tests (same as keys/ for this repo).
    const TEST_PRIVATE: &str = "-----BEGIN PRIVATE KEY-----\n\
MC4CAQAwBQYDK2VwBCIEIARxMvy1pXhFqAkixO0h/JmYBazhoTfUqtLHexEp/jG1\n\
-----END PRIVATE KEY-----\n";

    #[test]
    fn issue_and_verify_roundtrip() {
        let jwt = issue_license(TEST_PRIVATE.as_bytes(), "Acme", 30, "standard", None).unwrap();
        let c = verify_license(&jwt, Some("ignored-when-unbound"), true).unwrap();
        assert_eq!(c.sub, "Acme");
        assert_eq!(c.edition, "standard");
    }

    #[test]
    fn install_id_binding() {
        let jwt = issue_license(
            TEST_PRIVATE.as_bytes(),
            "Acme",
            30,
            "standard",
            Some("inst-1"),
        )
        .unwrap();
        assert!(verify_license(&jwt, Some("inst-1"), true).is_ok());
        assert!(verify_license(&jwt, Some("inst-2"), true).is_err());
    }
}
