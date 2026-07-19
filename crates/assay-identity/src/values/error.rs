use std::{error::Error, fmt};

pub(super) const MAX_CLAIM_VALUE_LENGTH: usize = 512;
pub(super) const MAX_URL_LENGTH: usize = 2048;
pub(super) const MAX_CLAIM_NAME_LENGTH: usize = 128;

macro_rules! serde_via_try_from {
    ($name:ident) => {
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Self::try_from(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(super) use serde_via_try_from;

/// A validation error that never echoes the rejected value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityError {
    value_kind: &'static str,
    reason: &'static str,
}

impl IdentityError {
    pub(crate) const fn new(value_kind: &'static str, reason: &'static str) -> Self {
        Self { value_kind, reason }
    }

    /// Returns the stable name of the rejected value type.
    pub const fn value_kind(&self) -> &'static str {
        self.value_kind
    }

    /// Returns a non-sensitive validation reason.
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.value_kind, self.reason)
    }
}

impl Error for IdentityError {}

pub(super) fn validate_claim_text(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("expected a non-empty value");
    }
    if value.len() > MAX_CLAIM_VALUE_LENGTH {
        return Err("value exceeds the maximum length");
    }
    if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err("value contains control characters");
    }
    Ok(())
}

pub(super) fn validate_https_url(value: &str, allow_query: bool) -> Result<(), &'static str> {
    if value.len() > MAX_URL_LENGTH {
        return Err("value exceeds the maximum length");
    }
    let Some(rest) = value.strip_prefix("https://") else {
        return Err("expected an https URL");
    };
    if rest.is_empty() {
        return Err("expected a host after the scheme");
    }
    if value
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err("value contains whitespace or control characters");
    }
    if value.contains('#') {
        return Err("a fragment is not allowed");
    }
    if !allow_query && value.contains('?') {
        return Err("a query is not allowed");
    }
    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return Err("expected a non-empty host");
    }
    if authority.contains('@') {
        return Err("userinfo is not allowed in the authority");
    }
    Ok(())
}
