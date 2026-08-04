//! Product license state: install id, 30-day trial, commercial JWT.

use crate::db::Db;
use chrono::{DateTime, Duration, Utc};
use eventide_license::{verify_license, LicenseClaims, TRIAL_DAYS};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub const INSTALL_ID_KEY: &str = "install_id";
pub const TRIAL_STARTED_KEY: &str = "trial_started_at";
pub const LICENSE_KEY: &str = "license";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseKind {
    Trial,
    Licensed,
    Expired,
}

#[derive(Debug, Clone, Serialize)]
pub struct LicenseSnapshot {
    pub kind: LicenseKind,
    /// Whether configuration writes are allowed.
    pub writable: bool,
    pub install_id: String,
    pub customer: Option<String>,
    pub edition: Option<String>,
    pub expires_at: Option<String>,
    pub days_left: Option<i64>,
    pub trial_started_at: Option<String>,
    pub reason: Option<String>,
    /// Raw JWT present in storage (not validated as currently active).
    pub has_license_token: bool,
}

pub struct LicenseGate {
    inner: RwLock<LicenseSnapshot>,
}

impl LicenseGate {
    pub fn from_db(db: &Db) -> anyhow::Result<Arc<Self>> {
        let snap = load_and_evaluate(db)?;
        tracing::info!(
            kind = ?snap.kind,
            writable = snap.writable,
            days_left = ?snap.days_left,
            install_id = %snap.install_id,
            "product license evaluated"
        );
        Ok(Arc::new(Self {
            inner: RwLock::new(snap),
        }))
    }

    pub fn snapshot(&self) -> LicenseSnapshot {
        self.inner
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| LicenseSnapshot {
                kind: LicenseKind::Expired,
                writable: false,
                install_id: String::new(),
                customer: None,
                edition: None,
                expires_at: None,
                days_left: None,
                trial_started_at: None,
                reason: Some("license state unavailable".into()),
                has_license_token: false,
            })
    }

    pub fn is_writable(&self) -> bool {
        self.snapshot().writable
    }

    pub fn refresh(&self, db: &Db) -> anyhow::Result<LicenseSnapshot> {
        let snap = load_and_evaluate(db)?;
        if let Ok(mut g) = self.inner.write() {
            *g = snap.clone();
        }
        Ok(snap)
    }

    pub fn import(&self, db: &Db, token: &str) -> anyhow::Result<LicenseSnapshot> {
        let install_id = ensure_install_id(db)?;
        // Reject expired / bad signature / wrong install up front.
        let claims = verify_license(token, Some(&install_id), true)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        db.set_kv(LICENSE_KEY, token.trim())?;
        tracing::info!(
            customer = %claims.sub,
            edition = %claims.edition,
            exp = %claims.expires_at().to_rfc3339(),
            "commercial license imported"
        );
        self.refresh(db)
    }

    pub fn clear_commercial(&self, db: &Db) -> anyhow::Result<LicenseSnapshot> {
        db.delete_kv(LICENSE_KEY)?;
        self.refresh(db)
    }
}

fn ensure_install_id(db: &Db) -> anyhow::Result<String> {
    if let Some(id) = db.get_kv(INSTALL_ID_KEY)? {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = Uuid::new_v4().to_string();
    db.set_kv(INSTALL_ID_KEY, &id)?;
    Ok(id)
}

fn ensure_trial_started(db: &Db) -> anyhow::Result<DateTime<Utc>> {
    if let Some(s) = db.get_kv(TRIAL_STARTED_KEY)? {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
            return Ok(dt.with_timezone(&Utc));
        }
    }
    let now = Utc::now();
    db.set_kv(TRIAL_STARTED_KEY, &now.to_rfc3339())?;
    Ok(now)
}

fn days_left_until(exp: DateTime<Utc>) -> i64 {
    let secs = (exp - Utc::now()).num_seconds();
    // ceil-ish days for UX: 1 hour left => 1 day
    if secs <= 0 {
        0
    } else {
        (secs + 86_399) / 86_400
    }
}

fn load_and_evaluate(db: &Db) -> anyhow::Result<LicenseSnapshot> {
    let install_id = ensure_install_id(db)?;

    let token = db
        .get_kv(LICENSE_KEY)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let has_license_token = token.is_some();

    if let Some(ref jwt) = token {
        match verify_license(jwt, Some(&install_id), true) {
            Ok(claims) => {
                let trial_started_s = db
                    .get_kv(TRIAL_STARTED_KEY)?
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                return Ok(licensed_ok(
                    install_id,
                    claims,
                    trial_started_s,
                    true,
                ));
            }
            Err(eventide_license::VerifyError::Expired) => {
                // Fall through to trial if still valid; else expired commercial.
                if let Ok(claims) = verify_license(jwt, Some(&install_id), false) {
                    let trial_started = ensure_trial_started(db)?;
                    let trial_ends = trial_started + Duration::days(TRIAL_DAYS);
                    let trial_started_s = trial_started.to_rfc3339();
                    let now = Utc::now();
                    if now < trial_ends {
                        return Ok(trial_ok(install_id, trial_started_s, trial_ends, true));
                    }
                    let exp_s = claims.expires_at().to_rfc3339();
                    return Ok(LicenseSnapshot {
                        kind: LicenseKind::Expired,
                        writable: false,
                        install_id,
                        customer: Some(claims.sub),
                        edition: Some(claims.edition),
                        expires_at: Some(exp_s),
                        days_left: Some(0),
                        trial_started_at: Some(trial_started_s),
                        reason: Some("许可证已过期，当前为只读宽限".into()),
                        has_license_token,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "stored license invalid; using trial/expiry path");
            }
        }
    }

    let trial_started = ensure_trial_started(db)?;
    let trial_ends = trial_started + Duration::days(TRIAL_DAYS);
    let trial_started_s = trial_started.to_rfc3339();
    let now = Utc::now();
    if now < trial_ends {
        Ok(trial_ok(
            install_id,
            trial_started_s,
            trial_ends,
            has_license_token,
        ))
    } else {
        Ok(LicenseSnapshot {
            kind: LicenseKind::Expired,
            writable: false,
            install_id,
            customer: None,
            edition: None,
            expires_at: Some(trial_ends.to_rfc3339()),
            days_left: Some(0),
            trial_started_at: Some(trial_started_s),
            reason: Some("试用已结束，请导入许可证（当前只读宽限）".into()),
            has_license_token,
        })
    }
}

fn licensed_ok(
    install_id: String,
    claims: LicenseClaims,
    trial_started_at: Option<String>,
    has_license_token: bool,
) -> LicenseSnapshot {
    let exp = claims.expires_at();
    LicenseSnapshot {
        kind: LicenseKind::Licensed,
        writable: true,
        install_id,
        customer: Some(claims.sub),
        edition: Some(claims.edition),
        expires_at: Some(exp.to_rfc3339()),
        days_left: Some(days_left_until(exp)),
        trial_started_at,
        reason: None,
        has_license_token,
    }
}

fn trial_ok(
    install_id: String,
    trial_started_at: String,
    trial_ends: DateTime<Utc>,
    has_license_token: bool,
) -> LicenseSnapshot {
    LicenseSnapshot {
        kind: LicenseKind::Trial,
        writable: true,
        install_id,
        customer: None,
        edition: Some("trial".into()),
        expires_at: Some(trial_ends.to_rfc3339()),
        days_left: Some(days_left_until(trial_ends)),
        trial_started_at: Some(trial_started_at),
        reason: None,
        has_license_token,
    }
}
