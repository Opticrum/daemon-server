//! In-memory wallet unlock sessions for the admin panel.
//!
//! After a successful unlock, a random token is stored here (along with the
//! password, kept only in RAM) and issued to the browser as an HttpOnly cookie.
//! Sessions expire after one hour.

use rand::Rng;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SESSION_TTL_SECS: u64 = 3600;
pub const SESSION_COOKIE: &str = "opticrum_wallet_session";

struct ActiveSession {
    token: String,
    expires_at: SystemTime,
    password: String,
}

/// Tracks at most one active admin unlock session (single-operator server).
#[derive(Default)]
pub struct WalletSessionManager {
    inner: Mutex<Option<ActiveSession>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStatus {
    pub active: bool,
    pub expires_at: Option<String>,
    pub remaining_secs: u64,
}

impl WalletSessionManager {
    /// Create a new session and return its token.
    pub fn create(&self, password: String) -> String {
        let token = new_token();
        let expires_at = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECS);
        *self.inner.lock().expect("session lock") = Some(ActiveSession {
            token: token.clone(),
            expires_at,
            password,
        });
        token
    }

    /// Return the session password when the token is valid and not expired.
    ///
    /// On success the expiry is extended by `SESSION_TTL_SECS` from now
    /// (sliding expiration), so an active session stays alive without
    /// requiring a periodic re-unlock.
    pub fn password_for(&self, token: &str) -> Option<String> {
        let mut guard = self.inner.lock().ok()?;
        let session = guard.as_mut()?;
        if SystemTime::now() >= session.expires_at {
            *guard = None;
            return None;
        }
        if session.token != token {
            return None;
        }
        // Slide the expiration window on each successful access.
        session.expires_at = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECS);
        Some(session.password.clone())
    }

    /// Extend the session TTL when the token is valid, without returning the
    /// password. Used by `ensure_signer_from_session` to keep the session
    /// alive while the user browses, even when the signer is already loaded.
    ///
    /// Returns `true` if the session was touched (token valid, not expired).
    pub fn touch(&self, token: &str) -> bool {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        let Some(session) = guard.as_mut() else {
            return false;
        };
        if SystemTime::now() >= session.expires_at {
            *guard = None;
            return false;
        }
        if session.token != token {
            return false;
        }
        session.expires_at = SystemTime::now() + Duration::from_secs(SESSION_TTL_SECS);
        true
    }

    pub fn status(&self, token: Option<&str>) -> SessionStatus {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return SessionStatus {
                    active: false,
                    expires_at: None,
                    remaining_secs: 0,
                }
            }
        };

        let Some(session) = guard.as_mut() else {
            return SessionStatus {
                active: false,
                expires_at: None,
                remaining_secs: 0,
            };
        };

        if SystemTime::now() >= session.expires_at {
            *guard = None;
            return SessionStatus {
                active: false,
                expires_at: None,
                remaining_secs: 0,
            };
        }

        let Some(token) = token else {
            return SessionStatus {
                active: false,
                expires_at: None,
                remaining_secs: 0,
            };
        };

        if session.token != token {
            return SessionStatus {
                active: false,
                expires_at: None,
                remaining_secs: 0,
            };
        }

        let remaining = session
            .expires_at
            .duration_since(SystemTime::now())
            .unwrap_or_default()
            .as_secs();

        SessionStatus {
            active: true,
            expires_at: Some(format_system_time(session.expires_at)),
            remaining_secs: remaining,
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }
}

fn new_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    hex::encode(bytes)
}

fn format_system_time(ts: SystemTime) -> String {
    let secs = ts.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let (year, month, day) = civil_from_days(days as i64);
    let hours = (time_of_day / 3600) % 24;
    let minutes = (time_of_day / 60) % 60;
    let seconds = time_of_day % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_validate_session() {
        let mgr = WalletSessionManager::default();
        let token = mgr.create("secret".into());
        assert!(mgr.password_for(&token).is_some());
        let status = mgr.status(Some(&token));
        assert!(status.active);
        assert!(status.remaining_secs > 3500);
    }

    #[test]
    fn wrong_token_rejected() {
        let mgr = WalletSessionManager::default();
        let token = mgr.create("secret".into());
        assert!(mgr.password_for("wrong").is_none());
        assert!(!mgr.status(Some("wrong")).active);
        assert!(mgr.status(Some(&token)).active);
    }

    #[test]
    fn clear_removes_session() {
        let mgr = WalletSessionManager::default();
        let token = mgr.create("secret".into());
        mgr.clear();
        assert!(mgr.password_for(&token).is_none());
        assert!(!mgr.status(Some(&token)).active);
    }
}
