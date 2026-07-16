use std::{collections::BTreeMap, fmt};

use crate::values::ClaimName;

/// A wall-clock instant in seconds since the Unix epoch, injected for determinism.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnixTime(i64);

impl UnixTime {
    /// Wraps a Unix timestamp in seconds.
    pub const fn from_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// Returns the timestamp in seconds.
    pub const fn as_seconds(self) -> i64 {
        self.0
    }
}

/// Injected clock port; a fixed test clock keeps time validation deterministic.
pub trait Clock {
    /// Returns the current instant.
    fn now(&self) -> UnixTime;
}

/// The raw upstream compact identity token. It never derives Debug, Display, or serde.
#[derive(Clone)]
pub struct UpstreamIdToken(String);

impl UpstreamIdToken {
    /// Wraps a compact upstream token received during code exchange.
    pub fn new(compact: String) -> Self {
        Self(compact)
    }

    /// Reveals the compact token only to a signature-verifier port implementation.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for UpstreamIdToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("UpstreamIdToken(<redacted>)")
    }
}

/// The authenticated claim set a signature verifier returns before policy validation.
///
/// A verifier proves signature and reports the algorithm; it enforces no policy.
/// Claims are raw strings so no provider-specific role enum is imported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedClaims {
    issuer: String,
    subject: String,
    audiences: Vec<String>,
    authorized_party: Option<String>,
    expiration: i64,
    not_before: Option<i64>,
    issued_at: i64,
    nonce: Option<String>,
    string_claims: BTreeMap<String, Vec<String>>,
}

impl VerifiedClaims {
    /// Returns the raw issuer claim.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Returns the raw subject claim.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Returns the raw audience list.
    pub fn audiences(&self) -> &[String] {
        &self.audiences
    }

    /// Returns the authorized party claim when present.
    pub fn authorized_party(&self) -> Option<&str> {
        self.authorized_party.as_deref()
    }

    /// Returns the expiration time in seconds.
    pub const fn expiration(&self) -> i64 {
        self.expiration
    }

    /// Returns the not-before time in seconds when present.
    pub const fn not_before(&self) -> Option<i64> {
        self.not_before
    }

    /// Returns the issued-at time in seconds.
    pub const fn issued_at(&self) -> i64 {
        self.issued_at
    }

    /// Returns the nonce claim when present.
    pub fn nonce(&self) -> Option<&str> {
        self.nonce.as_deref()
    }

    /// Returns the normalized values of one configured string-or-array claim.
    pub fn claim_values(&self, name: &ClaimName) -> &[String] {
        self.string_claims
            .get(name.as_str())
            .map_or(&[], Vec::as_slice)
    }
}

/// Builder for [`VerifiedClaims`] used by verifier ports and their test doubles.
#[derive(Clone, Debug)]
pub struct VerifiedClaimsBuilder {
    issuer: String,
    subject: String,
    audiences: Vec<String>,
    authorized_party: Option<String>,
    expiration: i64,
    not_before: Option<i64>,
    issued_at: i64,
    nonce: Option<String>,
    string_claims: BTreeMap<String, Vec<String>>,
}

impl VerifiedClaimsBuilder {
    /// Starts a claim set with the mandatory issuer, subject, and time claims.
    pub fn new(issuer: &str, subject: &str, expiration: i64, issued_at: i64) -> Self {
        Self {
            issuer: issuer.to_owned(),
            subject: subject.to_owned(),
            audiences: Vec::new(),
            authorized_party: None,
            expiration,
            not_before: None,
            issued_at,
            nonce: None,
            string_claims: BTreeMap::new(),
        }
    }

    /// Adds one audience entry.
    #[must_use]
    pub fn audience(mut self, audience: &str) -> Self {
        self.audiences.push(audience.to_owned());
        self
    }

    /// Sets the authorized party claim.
    #[must_use]
    pub fn authorized_party(mut self, party: &str) -> Self {
        self.authorized_party = Some(party.to_owned());
        self
    }

    /// Sets the not-before time.
    #[must_use]
    pub const fn not_before(mut self, not_before: i64) -> Self {
        self.not_before = Some(not_before);
        self
    }

    /// Sets the nonce claim bound to one authorization transaction.
    #[must_use]
    pub fn nonce(mut self, nonce: &str) -> Self {
        self.nonce = Some(nonce.to_owned());
        self
    }

    /// Adds one normalized string-or-array claim value.
    #[must_use]
    pub fn string_claim(mut self, name: &str, value: &str) -> Self {
        self.string_claims
            .entry(name.to_owned())
            .or_default()
            .push(value.to_owned());
        self
    }

    /// Finalizes the authenticated claim set.
    pub fn build(self) -> VerifiedClaims {
        VerifiedClaims {
            issuer: self.issuer,
            subject: self.subject,
            audiences: self.audiences,
            authorized_party: self.authorized_party,
            expiration: self.expiration,
            not_before: self.not_before,
            issued_at: self.issued_at,
            nonce: self.nonce,
            string_claims: self.string_claims,
        }
    }
}
