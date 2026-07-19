//! Validated rule and policy version identifiers.
//!
//! Split from `lib.rs` so identifier canonicalization stays separate from the
//! classification results and policy evaluation that reference identifiers.

use crate::error::ClassificationError;

/// A stable rule identifier scoped by a [`PolicyVersion`].
///
/// Individual rule IDs need not repeat the policy version. The policy identity
/// carried by every [`crate::FileClassification`] versions their meaning.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleId(String);

impl RuleId {
    pub(crate) fn built_in(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    /// Creates a policy rule identifier suitable for external versioned
    /// policy adapters.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(ClassificationError::rule_id(
                "expected a non-empty identifier of at most 128 characters",
            ));
        }
        let bytes = value.as_bytes();
        if !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_lowercase()
                && !bytes[bytes.len() - 1].is_ascii_digit()
            || value.contains("..")
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(ClassificationError::rule_id(
                "expected a canonical lowercase versioned identifier",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the canonical rule identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated identity of one complete classification policy.
///
/// A canonical policy identity ends in a positive numeric version, such as
/// `file-classifier-1` or `deployment-policy-v7`. Future CFG-002 rule-set
/// hashing can combine this identity with normalized external policy inputs;
/// this identity is provenance and is not itself a configuration hash.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PolicyVersion(String);

impl PolicyVersion {
    pub(crate) fn built_in() -> Self {
        Self(crate::BUILT_IN_RULE_SET_VERSION.to_owned())
    }

    /// Creates an explicitly versioned canonical policy identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(ClassificationError::policy_version(
                "expected a non-empty identity of at most 128 characters",
            ));
        }
        let bytes = value.as_bytes();
        if !bytes[0].is_ascii_lowercase()
            || !bytes[bytes.len() - 1].is_ascii_digit()
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(ClassificationError::policy_version(
                "expected a canonical lowercase identity",
            ));
        }
        let mut previous_was_separator = false;
        for byte in bytes {
            let is_separator = matches!(byte, b'.' | b'_' | b'-');
            if is_separator && previous_was_separator {
                return Err(ClassificationError::policy_version(
                    "adjacent separators are not allowed",
                ));
            }
            previous_was_separator = is_separator;
        }
        let version = value.rsplit('-').next().unwrap_or_default();
        let digits = version.strip_prefix('v').unwrap_or(version);
        if digits.is_empty()
            || digits.starts_with('0')
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ClassificationError::policy_version(
                "expected a positive numeric version suffix",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the canonical policy identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
