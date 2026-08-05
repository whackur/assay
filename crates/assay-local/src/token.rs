//! Named-environment-variable resolution for a least-privilege GitHub PAT.
//!
//! The token value never reaches a command argument, log, result, error, or
//! stored record. Only the variable name is retained; the value reaches the
//! transport boundary and nothing else.

use std::collections::BTreeMap;
use std::fmt;

/// Name of an environment variable that may hold a GitHub PAT.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubTokenEnvVar(String);

/// Non-sensitive validation failure that never echoes a token value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenEnvVarError {
    reason: &'static str,
}

impl TokenEnvVarError {
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for TokenEnvVarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid github token env var: {}", self.reason)
    }
}

impl std::error::Error for TokenEnvVarError {}

impl GithubTokenEnvVar {
    /// Parses a POSIX-style environment variable name. Argument is the name, never the token value.
    pub fn parse(name: &str) -> Result<Self, TokenEnvVarError> {
        let mut bytes = name.bytes();
        let first = bytes.next().ok_or(TokenEnvVarError {
            reason: "name is empty",
        })?;
        if !(first.is_ascii_uppercase() || first == b'_') {
            return Err(TokenEnvVarError {
                reason: "name must start with a letter or underscore",
            });
        }
        if name
            .bytes()
            .any(|byte| !(byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'))
        {
            return Err(TokenEnvVarError {
                reason: "name allows only A-Z, 0-9, and underscore",
            });
        }
        Ok(Self(name.to_owned()))
    }

    /// Never the token value.
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// GitHub token value that never appears in `Debug`, `Display`, serialization,
/// logs, results, or error text. Bytes leave only through
/// [`SecretToken::expose_for_authorization`] at the transport boundary.
pub struct SecretToken(String);

impl SecretToken {
    pub fn from_value(value: String) -> Self {
        Self(value)
    }

    /// Exposes the token only for constructing a transport authorization header.
    pub fn expose_for_authorization(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretToken([redacted])")
    }
}

/// Source of environment variable values, injected for determinism.
pub trait TokenEnvironment {
    fn read(&self, var: &GithubTokenEnvVar) -> Option<SecretToken>;
}

/// Reads token values from the live process environment by name.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessEnvironment;

impl TokenEnvironment for ProcessEnvironment {
    fn read(&self, var: &GithubTokenEnvVar) -> Option<SecretToken> {
        std::env::var(var.name()).ok().map(SecretToken::from_value)
    }
}

/// Deterministic in-memory environment for tests and fixtures.
#[derive(Clone, Debug, Default)]
pub struct MapEnvironment {
    values: BTreeMap<String, String>,
}

impl MapEnvironment {
    pub fn with(mut self, name: &str, value: &str) -> Self {
        self.values.insert(name.to_owned(), value.to_owned());
        self
    }
}

impl TokenEnvironment for MapEnvironment {
    fn read(&self, var: &GithubTokenEnvVar) -> Option<SecretToken> {
        self.values
            .get(var.name())
            .cloned()
            .map(SecretToken::from_value)
    }
}

/// Failure to resolve a configured token variable. Reports the variable name, never the value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenResolutionError {
    var: String,
}

impl TokenResolutionError {
    pub fn variable(&self) -> &str {
        &self.var
    }
}

impl fmt::Display for TokenResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "github token environment variable {} is not set",
            self.var
        )
    }
}

impl std::error::Error for TokenResolutionError {}

/// Resolves a token value from the injected environment. Failure carries only the variable name.
pub fn resolve_token(
    environment: &dyn TokenEnvironment,
    var: &GithubTokenEnvVar,
) -> Result<SecretToken, TokenResolutionError> {
    environment.read(var).ok_or_else(|| TokenResolutionError {
        var: var.name().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_posix_names() {
        assert!(GithubTokenEnvVar::parse("1TOKEN").is_err());
        assert!(GithubTokenEnvVar::parse("git-token").is_err());
        assert!(GithubTokenEnvVar::parse("").is_err());
        assert_eq!(
            GithubTokenEnvVar::parse("GITHUB_TOKEN").unwrap().name(),
            "GITHUB_TOKEN"
        );
    }

    #[test]
    fn secret_token_debug_is_redacted() {
        let secret = SecretToken::from_value("ghp_super_secret_value".to_owned());
        assert_eq!(format!("{secret:?}"), "SecretToken([redacted])");
        assert!(!format!("{secret:?}").contains("ghp_"));
    }

    #[test]
    fn missing_variable_error_names_variable_not_value() {
        let environment = MapEnvironment::default();
        let var = GithubTokenEnvVar::parse("GITHUB_TOKEN").unwrap();
        let error = resolve_token(&environment, &var).unwrap_err();
        assert_eq!(error.variable(), "GITHUB_TOKEN");
        assert!(!error.to_string().contains("ghp_"));
    }

    #[test]
    fn resolves_value_through_injected_environment() {
        let environment = MapEnvironment::default().with("GITHUB_TOKEN", "ghp_value");
        let var = GithubTokenEnvVar::parse("GITHUB_TOKEN").unwrap();
        let secret = resolve_token(&environment, &var).unwrap();
        assert_eq!(secret.expose_for_authorization(), "ghp_value");
    }
}
