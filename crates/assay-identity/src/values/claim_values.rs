use std::str::FromStr;

use super::error::{IdentityError, MAX_CLAIM_NAME_LENGTH, serde_via_try_from, validate_claim_text};

/// A validated subject identifier. The durable account key never uses email.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Subject(String);

impl Subject {
    /// Returns the subject claim value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Subject {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_claim_text(value).map_err(|reason| IdentityError::new("subject", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for Subject {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(Subject);

/// The audience Assay validates; a token issued only for another audience is rejected.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Audience(String);

impl Audience {
    /// Returns the expected audience value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Audience {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_claim_text(value).map_err(|reason| IdentityError::new("audience", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for Audience {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(Audience);

/// The Assay client identifier registered with the issuer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClientId(String);

impl ClientId {
    /// Returns the client identifier value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ClientId {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_claim_text(value).map_err(|reason| IdentityError::new("client_id", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for ClientId {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(ClientId);

/// A safe claim name (for example a configured role-bearing claim). Never a value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClaimName(String);

impl ClaimName {
    /// Returns the claim name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ClaimName {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = !value.is_empty()
            && value.len() <= MAX_CLAIM_NAME_LENGTH
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'/')
            });
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(IdentityError::new(
                "claim_name",
                "expected a safe claim name",
            ))
        }
    }
}

impl TryFrom<String> for ClaimName {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(ClaimName);
