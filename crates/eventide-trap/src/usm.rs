//! SNMPv3 User-based Security Model (RFC 3414 / 3826 / 7860) — receive path.

use crate::config::SnmpV3User;
use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, KeyIvInit as CbcKeyIvInit};
use cfb_mode::cipher::AsyncStreamCipher;
use des::Des;
use hmac::{Hmac, Mac};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use thiserror::Error;

type Aes128CfbDec = cfb_mode::Decryptor<Aes128>;
type DesCbcDec = cbc::Decryptor<Des>;
type HmacMd5 = Hmac<Md5>;
type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProtocol {
    None,
    Md5,
    Sha1,
    Sha256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivProtocol {
    None,
    Des,
    Aes128,
}

#[derive(Debug, Error)]
pub enum UsmError {
    #[error("unknown auth protocol `{0}`")]
    BadAuthProtocol(String),
    #[error("unknown priv protocol `{0}`")]
    BadPrivProtocol(String),
    #[error("user not configured")]
    UnknownUser,
    #[error("engine id mismatch")]
    EngineMismatch,
    #[error("authentication failed")]
    AuthFailed,
    #[error("privacy / decrypt failed: {0}")]
    PrivFailed(String),
    #[error("missing auth password")]
    MissingAuthPassword,
    #[error("missing priv password")]
    MissingPrivPassword,
    #[error("auth parameters missing from message")]
    AuthParamsNotFound,
}

impl AuthProtocol {
    pub fn parse(s: &str) -> Result<Self, UsmError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "noauth" => Ok(Self::None),
            "md5" => Ok(Self::Md5),
            "sha" | "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            other => Err(UsmError::BadAuthProtocol(other.into())),
        }
    }

    pub fn digest_len(self) -> usize {
        match self {
            Self::None => 0,
            Self::Md5 | Self::Sha1 => 12, // HMAC-*-96
            Self::Sha256 => 24,           // HMAC-SHA-256-192 (RFC 7860)
        }
    }

    pub fn key_len(self) -> usize {
        match self {
            Self::None => 0,
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }
}

impl PrivProtocol {
    pub fn parse(s: &str) -> Result<Self, UsmError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "nopriv" => Ok(Self::None),
            "des" => Ok(Self::Des),
            "aes" | "aes128" => Ok(Self::Aes128),
            other => Err(UsmError::BadPrivProtocol(other.into())),
        }
    }
}

pub struct ResolvedUser<'a> {
    pub user: &'a SnmpV3User,
    pub auth: AuthProtocol,
    pub privy: PrivProtocol,
}

pub fn resolve_user<'a>(
    users: &'a [SnmpV3User],
    username: &str,
    engine_id: &[u8],
) -> Result<ResolvedUser<'a>, UsmError> {
    let user = users
        .iter()
        .find(|u| u.user == username)
        .ok_or(UsmError::UnknownUser)?;
    if !user.engine_id.trim().is_empty() {
        let want = parse_engine_id_hex(&user.engine_id).map_err(|_| UsmError::EngineMismatch)?;
        if want.as_slice() != engine_id {
            return Err(UsmError::EngineMismatch);
        }
    }
    let auth = AuthProtocol::parse(&user.auth_protocol)?;
    let privy = PrivProtocol::parse(&user.priv_protocol)?;
    if auth != AuthProtocol::None && user.auth_password.is_empty() {
        return Err(UsmError::MissingAuthPassword);
    }
    if privy != PrivProtocol::None && user.priv_password.is_empty() {
        return Err(UsmError::MissingPrivPassword);
    }
    Ok(ResolvedUser { user, auth, privy })
}

pub fn parse_engine_id_hex(s: &str) -> Result<Vec<u8>, ()> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() % 2 != 0 || hex.is_empty() {
        return Err(());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

/// RFC 3414 / 7860 password → key, then localize with engine ID.
pub fn localize_key(auth: AuthProtocol, password: &str, engine_id: &[u8]) -> Vec<u8> {
    let ku = password_to_key(auth, password.as_bytes());
    localize(&ku, engine_id, auth)
}

fn password_to_key(auth: AuthProtocol, password: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return vec![0u8; auth.key_len()];
    }
    // 1 MiB of password repeated
    let mut buf = vec![0u8; 1_048_576];
    let mut offset = 0;
    while offset < buf.len() {
        let n = (buf.len() - offset).min(password.len());
        buf[offset..offset + n].copy_from_slice(&password[..n]);
        offset += n;
    }
    match auth {
        AuthProtocol::None => Vec::new(),
        AuthProtocol::Md5 => Md5::digest(&buf).to_vec(),
        AuthProtocol::Sha1 => Sha1::digest(&buf).to_vec(),
        AuthProtocol::Sha256 => Sha256::digest(&buf).to_vec(),
    }
}

fn localize(ku: &[u8], engine_id: &[u8], auth: AuthProtocol) -> Vec<u8> {
    let mut data = Vec::with_capacity(ku.len() * 2 + engine_id.len());
    data.extend_from_slice(ku);
    data.extend_from_slice(engine_id);
    data.extend_from_slice(ku);
    match auth {
        AuthProtocol::None => Vec::new(),
        AuthProtocol::Md5 => Md5::digest(&data).to_vec(),
        AuthProtocol::Sha1 => Sha1::digest(&data).to_vec(),
        AuthProtocol::Sha256 => Sha256::digest(&data).to_vec(),
    }
}

/// Verify msgAuthenticationParameters (whole-message HMAC with params zeroed).
pub fn verify_authentication(
    whole_msg: &[u8],
    auth_params: &[u8],
    auth_key: &[u8],
    auth: AuthProtocol,
) -> Result<(), UsmError> {
    if auth == AuthProtocol::None {
        return Ok(());
    }
    let expect_len = auth.digest_len();
    if auth_params.len() != expect_len {
        return Err(UsmError::AuthFailed);
    }
    let mut msg = whole_msg.to_vec();
    let pos = find_subslice(&msg, auth_params).ok_or(UsmError::AuthParamsNotFound)?;
    for b in &mut msg[pos..pos + auth_params.len()] {
        *b = 0;
    }
    let mac = compute_hmac(auth, auth_key, &msg)?;
    if mac.as_slice() != auth_params {
        return Err(UsmError::AuthFailed);
    }
    Ok(())
}

fn compute_hmac(auth: AuthProtocol, key: &[u8], data: &[u8]) -> Result<Vec<u8>, UsmError> {
    let trunc = auth.digest_len();
    let full = match auth {
        AuthProtocol::None => return Ok(Vec::new()),
        AuthProtocol::Md5 => {
            let mut mac = HmacMd5::new_from_slice(key).map_err(|_| UsmError::AuthFailed)?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha1 => {
            let mut mac = HmacSha1::new_from_slice(key).map_err(|_| UsmError::AuthFailed)?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        AuthProtocol::Sha256 => {
            let mut mac = HmacSha256::new_from_slice(key).map_err(|_| UsmError::AuthFailed)?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
    };
    Ok(full[..trunc.min(full.len())].to_vec())
}

/// Decrypt scoped PDU ciphertext.
pub fn decrypt_scoped_pdu(
    ciphertext: &[u8],
    priv_key: &[u8],
    privy: PrivProtocol,
    engine_boots: u32,
    engine_time: u32,
    priv_params: &[u8],
) -> Result<Vec<u8>, UsmError> {
    match privy {
        PrivProtocol::None => Ok(ciphertext.to_vec()),
        PrivProtocol::Des => decrypt_des(ciphertext, priv_key, priv_params),
        PrivProtocol::Aes128 => {
            decrypt_aes128(ciphertext, priv_key, engine_boots, engine_time, priv_params)
        }
    }
}

fn decrypt_des(ciphertext: &[u8], priv_key: &[u8], salt: &[u8]) -> Result<Vec<u8>, UsmError> {
    if priv_key.len() < 16 || salt.len() != 8 {
        return Err(UsmError::PrivFailed("DES key/salt length".into()));
    }
    if ciphertext.len() % 8 != 0 {
        return Err(UsmError::PrivFailed("DES ciphertext not 8-aligned".into()));
    }
    let mut pre_iv = [0u8; 8];
    for i in 0..8 {
        pre_iv[i] = priv_key[8 + i] ^ salt[i];
    }
    let mut buf = ciphertext.to_vec();
    DesCbcDec::new_from_slices(&priv_key[..8], &pre_iv)
        .map_err(|e| UsmError::PrivFailed(format!("DES init: {e}")))?
        .decrypt_padded_mut::<cipher::block_padding::NoPadding>(&mut buf)
        .map_err(|e| UsmError::PrivFailed(format!("DES CBC: {e}")))?;
    Ok(buf)
}

fn decrypt_aes128(
    ciphertext: &[u8],
    priv_key: &[u8],
    boots: u32,
    time: u32,
    salt: &[u8],
) -> Result<Vec<u8>, UsmError> {
    if priv_key.len() < 16 || salt.len() != 8 {
        return Err(UsmError::PrivFailed("AES key/salt length".into()));
    }
    let mut iv = [0u8; 16];
    iv[0..4].copy_from_slice(&boots.to_be_bytes());
    iv[4..8].copy_from_slice(&time.to_be_bytes());
    iv[8..16].copy_from_slice(salt);
    let mut buf = ciphertext.to_vec();
    use cfb_mode::cipher::KeyIvInit;
    let dec = Aes128CfbDec::new_from_slices(&priv_key[..16], &iv)
        .map_err(|e| UsmError::PrivFailed(format!("AES init: {e}")))?;
    dec.decrypt(&mut buf);
    Ok(buf)
}

/// Priv key localization uses the auth protocol's hash (RFC 3414 §2.6).
pub fn localize_priv_key(
    auth: AuthProtocol,
    priv_password: &str,
    engine_id: &[u8],
    privy: PrivProtocol,
) -> Vec<u8> {
    let need = match privy {
        PrivProtocol::None => return Vec::new(),
        PrivProtocol::Des | PrivProtocol::Aes128 => 16,
    };
    let hash_auth = if auth == AuthProtocol::None {
        AuthProtocol::Md5
    } else {
        auth
    };
    let mut key = localize_key(hash_auth, priv_password, engine_id);
    // Extend if needed (short hashes → AES/DES 16-byte key material).
    while key.len() < need {
        let extra = match hash_auth {
            AuthProtocol::Md5 => Md5::digest(&key).to_vec(),
            AuthProtocol::Sha1 => Sha1::digest(&key).to_vec(),
            AuthProtocol::Sha256 => Sha256::digest(&key).to_vec(),
            AuthProtocol::None => break,
        };
        key.extend_from_slice(&extra);
    }
    key.truncate(need);
    key
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_to_key_md5_len() {
        let k = password_to_key(AuthProtocol::Md5, b"maplesyrup");
        assert_eq!(k.len(), 16);
    }

    /// RFC 3414 Appendix A.3.1 sample (MD5 / maplesyrup / engineID).
    #[test]
    fn rfc3414_md5_localize_sample() {
        let engine = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ];
        let kul = localize_key(AuthProtocol::Md5, "maplesyrup", &engine);
        assert_eq!(
            kul,
            [
                0x52, 0x6f, 0x5e, 0xed, 0x9f, 0xcc, 0xe2, 0x6f, 0x89, 0x64, 0xc2, 0x93, 0x07, 0x87,
                0xd8, 0x2b
            ]
        );
    }

    #[test]
    fn auth_digest_lens() {
        assert_eq!(AuthProtocol::Md5.digest_len(), 12);
        assert_eq!(AuthProtocol::Sha256.digest_len(), 24);
    }

    #[test]
    fn parse_engine_hex() {
        let e = parse_engine_id_hex("80:00:1f:88:80").unwrap();
        assert_eq!(e, vec![0x80, 0x00, 0x1f, 0x88, 0x80]);
    }
}
