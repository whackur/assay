use std::fmt;

/// Stable reference name of a secret, never the secret value itself.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecretName(String);

impl SecretName {
    /// Validates a secret reference name; rejects empty or unsafe names.
    pub fn new(name: &str) -> Result<Self, SecretError> {
        let valid = !name.is_empty()
            && name.len() <= 128
            && name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'/')
            });
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(SecretError::InvalidName)
        }
    }

    /// Returns the reference name used to look the secret up.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A loaded API credential that never appears in Debug, Display, or serialization.
#[derive(Clone)]
pub struct ProviderSecret(String);

impl ProviderSecret {
    /// Wraps raw key material read from a secret store.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Exposes the key only to code that builds the outbound request header.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderSecret(<redacted>)")
    }
}

/// Redacted failure category for secret resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretError {
    InvalidName,
    NotFound,
    Unavailable,
}

/// Name-addressed secret store; a rotated key is read by the same name.
pub trait SecretStore {
    /// Loads current key material for one reference name from secret storage.
    fn load(&self, name: &SecretName) -> Result<ProviderSecret, SecretError>;
}
