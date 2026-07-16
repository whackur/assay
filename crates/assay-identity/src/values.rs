use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const MAX_CLAIM_VALUE_LENGTH: usize = 512;
const MAX_URL_LENGTH: usize = 2048;
const MAX_CLAIM_NAME_LENGTH: usize = 128;

macro_rules! serde_via_try_from {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::try_from(String::deserialize(deserializer)?).map_err(de::Error::custom)
            }
        }
    };
}

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

fn validate_claim_text(value: &str) -> Result<(), &'static str> {
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

fn validate_https_url(value: &str, allow_query: bool) -> Result<(), &'static str> {
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
