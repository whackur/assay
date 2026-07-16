//! Provider-agnostic identity boundary for Assay.
//!
//! Assay normalizes a successful upstream OIDC assertion into its own account,
//! opaque session, local role, and entitlement records. The durable account key
//! is the validated `(issuer, subject)` pair, never email. This crate keys on a
//! configured issuer only: no provider domain, claim convention, or role enum is
//! hard-coded, and it queries no upstream user database.
//!
//! Signature and JWKS crypto sit behind the [`SignatureVerifier`] port, and the
//! clock and entropy behind the [`Clock`] and [`EntropySource`] ports, so all
//! validation, session, and policy logic here stays pure and deterministic. No
//! network, filesystem, or process I/O happens in this crate.

#![forbid(unsafe_code)]

mod audit;
mod claims;
mod config;
mod encoding;
mod entitlements;
mod flow;
mod policy;
mod session;
mod values;
mod verification;

pub use audit::{AuditAction, AuditEvent};
pub use claims::{Clock, UnixTime, UpstreamIdToken, VerifiedClaims, VerifiedClaimsBuilder};
pub use config::{ConfigError, OidcDeploymentConfig, OidcDeploymentConfigBuilder};
pub use entitlements::{Entitlement, EntitlementPolicy, LocalRole};
pub use flow::{
    AuthorizationRedirect, AuthorizationStore, CallbackParams, EntropySource, FlowError, Nonce,
    PkceChallenge, PkceVerifier, State, VerifiedCallback,
};
pub use policy::{AdministratorMappingPolicy, RoleAssignment, RoleSource, TrustedAdminClaim};
pub use session::{Session, SessionError, SessionId, SessionSecret, SessionState};
pub use values::{
    AccountKey, Audience, ClaimName, ClientId, IdentityError, IssuerUrl, RedirectUri, Subject,
};
pub use verification::{
    SignatureError, SignatureVerifier, SigningAlgorithm, TokenValidator, ValidationError,
    VerifiedAssertion, VerifiedIdentity,
};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
