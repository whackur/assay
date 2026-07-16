use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    claims::{Clock, UpstreamIdToken, VerifiedClaims},
    config::OidcDeploymentConfig,
    values::{AccountKey, Subject},
};

/// Allowed asymmetric signing algorithms. `none` and symmetric HMAC are unrepresentable.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SigningAlgorithm {
    Rs256,
    Rs384,
    Rs512,
    Es256,
    Es384,
    Es512,
    Ps256,
    Ps384,
    Ps512,
}

/// Redacted signature-verification failure with no token or key material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureError {
    MalformedToken,
    UnknownKey,
    InvalidSignature,
    JwksUnavailable,
}

/// An authenticated assertion: the algorithm proven and the resulting claim set.
#[derive(Clone, Debug)]
pub struct VerifiedAssertion {
    algorithm: SigningAlgorithm,
    claims: VerifiedClaims,
}

impl VerifiedAssertion {
    /// Wraps the algorithm a verifier proved and the claims it recovered.
    pub const fn new(algorithm: SigningAlgorithm, claims: VerifiedClaims) -> Self {
        Self { algorithm, claims }
    }

    /// Returns the proven signing algorithm.
    pub const fn algorithm(&self) -> SigningAlgorithm {
        self.algorithm
    }

    /// Returns the authenticated claims prior to policy validation.
    pub const fn claims(&self) -> &VerifiedClaims {
        &self.claims
    }
}

/// Signature and JWKS crypto seam. The concrete verifier lives outside this crate.
///
/// It proves the token signature over the issuer JWKS and reports the algorithm
/// used; it does not decide whether that algorithm, issuer, audience, or time is
/// acceptable. All policy checks stay in [`TokenValidator`].
pub trait SignatureVerifier {
    /// Verifies one compact token and returns its authenticated assertion.
    fn verify(&self, token: &UpstreamIdToken) -> Result<VerifiedAssertion, SignatureError>;
}

/// Fail-closed identity-token validation outcome. Every variant redacts values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    SignatureRejected,
    DisallowedAlgorithm,
    IssuerMismatch,
    AudienceMismatch,
    AuthorizedPartyMismatch,
    Expired,
    NotYetValid,
    IssuedInFuture,
    MissingNonce,
    NonceMismatch,
    MissingSubject,
}

impl ValidationError {
    /// Returns the stable machine-readable code without any rejected value.
    pub const fn code(self) -> &'static str {
        match self {
            Self::SignatureRejected => "signature_rejected",
            Self::DisallowedAlgorithm => "disallowed_algorithm",
            Self::IssuerMismatch => "issuer_mismatch",
            Self::AudienceMismatch => "audience_mismatch",
            Self::AuthorizedPartyMismatch => "authorized_party_mismatch",
            Self::Expired => "expired",
            Self::NotYetValid => "not_yet_valid",
            Self::IssuedInFuture => "issued_in_future",
            Self::MissingNonce => "missing_nonce",
            Self::NonceMismatch => "nonce_mismatch",
            Self::MissingSubject => "missing_subject",
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ValidationError {}

/// A validated identity: the durable account key and its authenticated claims.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedIdentity {
    account_key: AccountKey,
    authentication_time: i64,
    claims: VerifiedClaims,
}

impl VerifiedIdentity {
    /// Returns the durable `(issuer, subject)` account key.
    pub const fn account_key(&self) -> &AccountKey {
        &self.account_key
    }

    /// Returns the issued-at time used as the authentication time.
    pub const fn authentication_time(&self) -> i64 {
        self.authentication_time
    }

    /// Returns the authenticated claims retained for local role mapping.
    pub const fn claims(&self) -> &VerifiedClaims {
        &self.claims
    }
}

/// Validates upstream identity tokens against one deployment's OIDC policy.
pub struct TokenValidator<'a> {
    config: &'a OidcDeploymentConfig,
}

impl<'a> TokenValidator<'a> {
    /// Binds a validator to one immutable deployment configuration.
    pub const fn new(config: &'a OidcDeploymentConfig) -> Self {
        Self { config }
    }

    /// Validates a token fail-closed, requiring the nonce from its transaction.
    pub fn validate(
        &self,
        verifier: &dyn SignatureVerifier,
        token: &UpstreamIdToken,
        expected_nonce: &str,
        clock: &dyn Clock,
    ) -> Result<VerifiedIdentity, ValidationError> {
        let assertion = verifier
            .verify(token)
            .map_err(|_| ValidationError::SignatureRejected)?;
        if !self.config.allows_algorithm(assertion.algorithm()) {
            return Err(ValidationError::DisallowedAlgorithm);
        }
        let claims = assertion.claims();
        self.check_issuer(claims)?;
        self.check_audience(claims)?;
        self.check_time(claims, clock.now().as_seconds())?;
        check_nonce(claims, expected_nonce)?;
        let subject =
            Subject::from_str(claims.subject()).map_err(|_| ValidationError::MissingSubject)?;
        let account_key = AccountKey::new(self.config.issuer().clone(), subject);
        Ok(VerifiedIdentity {
            account_key,
            authentication_time: claims.issued_at(),
            claims: claims.clone(),
        })
    }

    fn check_issuer(&self, claims: &VerifiedClaims) -> Result<(), ValidationError> {
        if claims.issuer() == self.config.issuer().as_str() {
            Ok(())
        } else {
            Err(ValidationError::IssuerMismatch)
        }
    }

    fn check_audience(&self, claims: &VerifiedClaims) -> Result<(), ValidationError> {
        let expected = self.config.expected_audience().as_str();
        if !claims.audiences().iter().any(|value| value == expected) {
            return Err(ValidationError::AudienceMismatch);
        }
        let client = self.config.client_id().as_str();
        // OIDC Core 3.1.3.7: a present azp must equal the client id regardless of
        // audience count, and multiple audiences require azp to be present.
        match claims.authorized_party() {
            Some(party) if party != client => {
                return Err(ValidationError::AuthorizedPartyMismatch);
            }
            None if claims.audiences().len() > 1 => {
                return Err(ValidationError::AuthorizedPartyMismatch);
            }
            _ => {}
        }
        Ok(())
    }

    fn check_time(&self, claims: &VerifiedClaims, now: i64) -> Result<(), ValidationError> {
        let leeway = self.config.clock_leeway_seconds();
        if now > claims.expiration().saturating_add(leeway) {
            return Err(ValidationError::Expired);
        }
        if let Some(not_before) = claims.not_before()
            && now < not_before.saturating_sub(leeway)
        {
            return Err(ValidationError::NotYetValid);
        }
        if now < claims.issued_at().saturating_sub(leeway) {
            return Err(ValidationError::IssuedInFuture);
        }
        Ok(())
    }
}

fn check_nonce(claims: &VerifiedClaims, expected_nonce: &str) -> Result<(), ValidationError> {
    match claims.nonce() {
        None => Err(ValidationError::MissingNonce),
        Some(nonce) if constant_time_eq(nonce.as_bytes(), expected_nonce.as_bytes()) => Ok(()),
        Some(_) => Err(ValidationError::NonceMismatch),
    }
}

/// Length-independent comparison to avoid leaking match position through timing.
pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}
