use std::ffi::OsString;

use assay_ai_evaluator::{ProviderSecret, SecretError, SecretName, SecretStore};

/// Environment-variable secret store: the first concrete [`SecretStore`].
///
/// The [`SecretName`] is the environment variable *name*; the value is read
/// only here and never appears in arguments, logs, results, or records.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvSecretStore;

impl EnvSecretStore {
    pub(crate) fn from_value(value: Option<OsString>) -> Result<ProviderSecret, SecretError> {
        match value {
            None => Err(SecretError::NotFound),
            Some(value) if value.is_empty() => Err(SecretError::NotFound),
            Some(value) => match value.into_string() {
                Ok(value) => Ok(ProviderSecret::new(value)),
                Err(_) => Err(SecretError::Unavailable),
            },
        }
    }
}

impl SecretStore for EnvSecretStore {
    fn load(&self, name: &SecretName) -> Result<ProviderSecret, SecretError> {
        Self::from_value(std::env::var_os(name.as_str()))
    }
}
