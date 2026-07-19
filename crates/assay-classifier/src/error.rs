//! Validation error type shared across classifier inputs.
//!
//! Split from `lib.rs` so path, identifier, confidence, and policy-version
//! validation can reuse one non-sensitive error without pulling in unrelated
//! domain types.

use std::{error::Error, fmt};

/// A validation error that does not retain or echo rejected path input.
#[derive(Clone, Eq, PartialEq)]
pub struct ClassificationError {
    value_kind: &'static str,
    reason: &'static str,
}

impl ClassificationError {
    pub(crate) fn portable_path(reason: &'static str) -> Self {
        Self {
            value_kind: "portable_path",
            reason,
        }
    }

    pub(crate) fn rule_id(reason: &'static str) -> Self {
        Self {
            value_kind: "rule_id",
            reason,
        }
    }

    pub(crate) fn confidence(reason: &'static str) -> Self {
        Self {
            value_kind: "confidence",
            reason,
        }
    }

    pub(crate) fn policy_version(reason: &'static str) -> Self {
        Self {
            value_kind: "policy_version",
            reason,
        }
    }

    /// Returns the stable input kind that failed validation.
    pub const fn value_kind(&self) -> &'static str {
        self.value_kind
    }

    /// Returns a non-sensitive reason that never includes the rejected value.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Debug for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClassificationError")
            .field("value_kind", &self.value_kind)
            .field("reason", &self.reason)
            .finish()
    }
}

impl fmt::Display for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.value_kind, self.reason)
    }
}

impl Error for ClassificationError {}
