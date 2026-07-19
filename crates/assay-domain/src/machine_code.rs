use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::error::{DomainValueError, MAX_CODE_LENGTH};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MachineCode(String);

fn validate_machine_code(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_CODE_LENGTH {
        return Err("expected a non-empty snake_case code of at most 64 characters");
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
        return Err("expected a snake_case code beginning with a letter");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || value.ends_with('_')
        || value.contains("__")
    {
        return Err("expected a canonical snake_case code");
    }
    Ok(())
}

impl TryFrom<String> for MachineCode {
    type Error = DomainValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_machine_code(&value)
            .map_err(|reason| DomainValueError::new("machine_code", reason))?;
        Ok(Self(value))
    }
}

impl FromStr for MachineCode {
    type Err = DomainValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for MachineCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for MachineCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

impl MachineCode {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
