use serde::{Deserialize, Serialize};

/// Credential-independent location of provider execution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProviderExecutionBoundary {
    Local,
    External,
}

/// Applicability of one project criterion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Applicable,
    PartiallyApplicable,
    NotApplicable,
}

/// Availability status of a provider judgment bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    Complete,
    Partial,
    Unavailable,
    Unsupported,
    Insufficient,
    Pending,
}

impl EvaluationStatus {
    /// Returns true when a judgment set may carry usable judgments.
    pub(crate) const fn is_usable(self) -> bool {
        matches!(self, Self::Complete | Self::Partial)
    }
}
