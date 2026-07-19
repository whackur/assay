use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::error::DomainValueError;

const MAX_CRITERION_LENGTH: usize = 100;

fn validate_rubric_criterion_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_CRITERION_LENGTH {
        return Err("expected a non-empty dotted identifier of at most 100 characters");
    }
    let mut segments = value.split('.');
    let mut count = 0;
    for segment in &mut segments {
        count += 1;
        let bytes = segment.as_bytes();
        if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
            return Err("expected each segment to begin with a lowercase letter");
        }
        if !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        {
            return Err("expected lowercase snake_case segments");
        }
    }
    if count < 2 {
        return Err("expected at least two dot-separated segments");
    }
    Ok(())
}

/// A validated dotted rubric criterion identifier such as
/// `substance.claim_implementation_fit`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RubricCriterionId(String);

impl RubricCriterionId {
    /// Returns the canonical dotted identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the leading dimension segment before the first dot.
    pub fn dimension_prefix(&self) -> &str {
        self.0.split('.').next().unwrap_or(&self.0)
    }
}

impl FromStr for RubricCriterionId {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_rubric_criterion_id(value)
            .map_err(|reason| DomainValueError::new("rubric_criterion_id", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for RubricCriterionId {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_rubric_criterion_id(&value)
            .map_err(|reason| DomainValueError::new("rubric_criterion_id", reason))?;
        Ok(Self(value))
    }
}

impl Serialize for RubricCriterionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RubricCriterionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
