use std::str::FromStr;

use super::error::{IdentityError, serde_via_try_from, validate_https_url};

/// An `https` OIDC issuer identifier supplied only through deployment configuration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IssuerUrl(String);

impl IssuerUrl {
    /// Returns the canonical issuer string compared exactly against token claims.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for IssuerUrl {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_https_url(value, false)
            .map_err(|reason| IdentityError::new("issuer_url", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for IssuerUrl {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(IssuerUrl);

/// An exact `https` redirect URI matched against the deployment allowlist.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RedirectUri(String);

impl RedirectUri {
    /// Returns the exact redirect URI string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RedirectUri {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_https_url(value, true)
            .map_err(|reason| IdentityError::new("redirect_uri", reason))?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for RedirectUri {
    type Error = IdentityError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

serde_via_try_from!(RedirectUri);
