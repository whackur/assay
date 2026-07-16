use std::fmt;

use crate::{
    claims::UnixTime, encoding::base64url_no_pad, flow::EntropySource, values::AccountKey,
};

const SESSION_SECRET_BYTES: usize = 32;
const SESSION_ID_BYTES: usize = 16;

/// The opaque session cookie value. Secret material; never derives Debug or serde.
///
/// This is the only bearer credential a browser holds. No upstream access or
/// refresh token is ever stored in a session or handed to the browser.
#[derive(Clone)]
pub struct SessionSecret(String);

impl SessionSecret {
    /// Reveals the cookie value only to a cookie writer or hashing store.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionSecret(<redacted>)")
    }
}

/// A non-secret session identifier for lineage and audit references.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(String);

impl SessionId {
    /// Returns the non-secret session identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The lifecycle state of an opaque Assay session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Active,
    Revoked,
}

/// Redacted session-operation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    NotActive,
    Expired,
}

fn generate(entropy: &dyn EntropySource, len: usize) -> String {
    let mut bytes = vec![0u8; len];
    entropy.fill(&mut bytes);
    base64url_no_pad(&bytes)
}

/// An opaque, rotatable Assay session that never holds an upstream token.
#[derive(Clone, Debug)]
pub struct Session {
    id: SessionId,
    secret: SessionSecret,
    account: AccountKey,
    state: SessionState,
    established_at: i64,
    expires_at: i64,
    rotation_count: u32,
}

impl Session {
    /// Establishes a fresh active session bound to an account key.
    pub fn establish(
        account: AccountKey,
        established_at: UnixTime,
        ttl_seconds: i64,
        entropy: &dyn EntropySource,
    ) -> Self {
        let established = established_at.as_seconds();
        Self {
            id: SessionId(generate(entropy, SESSION_ID_BYTES)),
            secret: SessionSecret(generate(entropy, SESSION_SECRET_BYTES)),
            account,
            state: SessionState::Active,
            established_at: established,
            expires_at: established.saturating_add(ttl_seconds),
            rotation_count: 0,
        }
    }

    /// Returns the non-secret session identifier.
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Returns the opaque cookie secret.
    pub const fn secret(&self) -> &SessionSecret {
        &self.secret
    }

    /// Returns the bound account key.
    pub const fn account(&self) -> &AccountKey {
        &self.account
    }

    /// Returns the current lifecycle state.
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Returns the absolute expiry in seconds.
    pub const fn expires_at(&self) -> i64 {
        self.expires_at
    }

    /// Returns the number of rotations applied to this session identity.
    pub const fn rotation_count(&self) -> u32 {
        self.rotation_count
    }

    /// Returns whether the session is usable at the given instant.
    pub fn is_valid(&self, now: UnixTime) -> bool {
        self.state == SessionState::Active && now.as_seconds() < self.expires_at
    }

    /// Rotates the cookie secret and extends expiry, keeping the session identity.
    pub fn rotate(
        &self,
        now: UnixTime,
        ttl_seconds: i64,
        entropy: &dyn EntropySource,
    ) -> Result<Self, SessionError> {
        if self.state != SessionState::Active {
            return Err(SessionError::NotActive);
        }
        if now.as_seconds() >= self.expires_at {
            return Err(SessionError::Expired);
        }
        Ok(Self {
            id: self.id.clone(),
            secret: SessionSecret(generate(entropy, SESSION_SECRET_BYTES)),
            account: self.account.clone(),
            state: SessionState::Active,
            established_at: self.established_at,
            expires_at: now.as_seconds().saturating_add(ttl_seconds),
            rotation_count: self.rotation_count.saturating_add(1),
        })
    }

    /// Revokes the session immediately.
    pub fn revoke(&mut self) {
        self.state = SessionState::Revoked;
    }
}
