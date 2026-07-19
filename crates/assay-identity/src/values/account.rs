use serde::{Deserialize, Serialize};

use super::claim_values::Subject;
use super::urls::IssuerUrl;

/// The durable account key: the validated `(issuer, subject)` pair, never email.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountKey {
    issuer: IssuerUrl,
    subject: Subject,
}

impl AccountKey {
    /// Builds an account key from a validated issuer and subject.
    pub const fn new(issuer: IssuerUrl, subject: Subject) -> Self {
        Self { issuer, subject }
    }

    /// Returns the issuer half of the key.
    pub const fn issuer(&self) -> &IssuerUrl {
        &self.issuer
    }

    /// Returns the subject half of the key.
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }
}
