use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::ContentHash;
use crate::error::{
    DomainValueError, GIT_SHA1_LENGTH, GIT_SHA256_LENGTH, is_lower_hex, validated_string_value,
};

fn validate_revision(value: &str) -> Result<(), &'static str> {
    if !matches!(value.len(), GIT_SHA1_LENGTH | GIT_SHA256_LENGTH) {
        return Err("expected a full 40- or 64-character object identifier");
    }
    if !is_lower_hex(value) {
        return Err("expected lowercase hexadecimal characters");
    }
    if value.bytes().all(|byte| byte == b'0') {
        return Err("the Git null object identifier is not an immutable revision");
    }
    Ok(())
}

fn validate_evidence_id(value: &str) -> Result<(), &'static str> {
    let mut parts = value.split(':');
    if parts.next() != Some("evidence") {
        return Err("expected the evidence namespace");
    }
    let remainder: Vec<_> = parts.collect();
    if remainder.len() < 2
        || !remainder
            .iter()
            .all(|part| crate::error::is_safe_component(part))
    {
        return Err("expected at least two safe identifier components");
    }
    Ok(())
}

fn validate_analysis_version(value: &str) -> Result<(), &'static str> {
    if !crate::error::is_safe_component(value) {
        return Err("expected a canonical lowercase version identifier");
    }
    Ok(())
}

validated_string_value!(RevisionId, "revision_id", validate_revision);
validated_string_value!(EvidenceId, "evidence_id", validate_evidence_id);
validated_string_value!(
    AnalysisVersion,
    "analysis_version",
    validate_analysis_version
);

/// A SHA-256 digest of the complete effective analysis rule set.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleSetHash(ContentHash);

impl RuleSetHash {
    /// Returns the canonical `sha256:<digest>` representation.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for RuleSetHash {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ContentHash::from_str(value)
            .map(Self)
            .map_err(|error| DomainValueError::new("rule_set_hash", error.reason()))
    }
}

impl TryFrom<String> for RuleSetHash {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ContentHash::try_from(value)
            .map(Self)
            .map_err(|error| DomainValueError::new("rule_set_hash", error.reason()))
    }
}

impl Serialize for RuleSetHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RuleSetHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
