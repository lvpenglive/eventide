//! In-memory login brute-force protection (per IP + per username).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct LoginLimitConfig {
    pub max_failures: u32,
    pub window: Duration,
    pub lockout: Duration,
}

impl Default for LoginLimitConfig {
    fn default() -> Self {
        Self {
            max_failures: 5,
            window: Duration::from_secs(15 * 60),
            lockout: Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug, Default)]
struct Bucket {
    failures: u32,
    window_start: Option<Instant>,
    locked_until: Option<Instant>,
}

pub struct LoginLimiter {
    cfg: LoginLimitConfig,
    inner: Mutex<HashMap<String, Bucket>>,
}

impl LoginLimiter {
    pub fn new(cfg: LoginLimitConfig) -> Self {
        Self {
            cfg,
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Returns Ok(()) if allowed, or Err(seconds remaining in lockout).
    pub fn check(&self, ip: &str, username: &str) -> Result<(), u64> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        self.purge_locked(&mut guard, now);
        for key in [format!("ip:{ip}"), format!("user:{}", username.to_ascii_lowercase())] {
            if let Some(b) = guard.get(&key) {
                if let Some(until) = b.locked_until {
                    if until > now {
                        return Err((until - now).as_secs().max(1));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn record_failure(&self, ip: &str, username: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        for key in [format!("ip:{ip}"), format!("user:{}", username.to_ascii_lowercase())] {
            self.bump(&mut guard, &key, now);
        }
    }

    pub fn record_success(&self, ip: &str, username: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(&format!("user:{}", username.to_ascii_lowercase()));
        // Keep IP soft history but clear lockout on success from this user path.
        if let Some(b) = guard.get_mut(&format!("ip:{ip}")) {
            b.locked_until = None;
            b.failures = 0;
            b.window_start = None;
        }
    }

    fn bump(&self, map: &mut HashMap<String, Bucket>, key: &str, now: Instant) {
        let b = map.entry(key.to_string()).or_default();
        if let Some(until) = b.locked_until {
            if until > now {
                return;
            }
            b.locked_until = None;
            b.failures = 0;
            b.window_start = None;
        }
        match b.window_start {
            Some(start) if now.duration_since(start) <= self.cfg.window => {}
            _ => {
                b.window_start = Some(now);
                b.failures = 0;
            }
        }
        b.failures = b.failures.saturating_add(1);
        if b.failures >= self.cfg.max_failures {
            b.locked_until = Some(now + self.cfg.lockout);
            b.failures = 0;
            b.window_start = None;
        }
    }

    fn purge_locked(&self, map: &mut HashMap<String, Bucket>, now: Instant) {
        map.retain(|_, b| {
            if let Some(until) = b.locked_until {
                if until <= now {
                    b.locked_until = None;
                }
            }
            b.locked_until.is_some()
                || b.failures > 0
                || b.window_start
                    .map(|s| now.duration_since(s) <= self.cfg.window)
                    .unwrap_or(false)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_max_failures() {
        let lim = LoginLimiter::new(LoginLimitConfig {
            max_failures: 3,
            window: Duration::from_secs(60),
            lockout: Duration::from_secs(30),
        });
        assert!(lim.check("1.1.1.1", "admin").is_ok());
        lim.record_failure("1.1.1.1", "admin");
        lim.record_failure("1.1.1.1", "admin");
        assert!(lim.check("1.1.1.1", "admin").is_ok());
        lim.record_failure("1.1.1.1", "admin");
        let err = lim.check("1.1.1.1", "admin").unwrap_err();
        assert!(err >= 1);
    }

    #[test]
    fn success_clears_user_bucket() {
        let lim = LoginLimiter::new(LoginLimitConfig {
            max_failures: 2,
            window: Duration::from_secs(60),
            lockout: Duration::from_secs(30),
        });
        lim.record_failure("2.2.2.2", "bob");
        lim.record_success("2.2.2.2", "bob");
        lim.record_failure("2.2.2.2", "bob");
        assert!(lim.check("2.2.2.2", "bob").is_ok());
    }
}
