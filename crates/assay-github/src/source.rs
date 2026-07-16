use std::{error::Error, fmt};

const MAX_OWNER_BYTES: usize = 39;
const MAX_REPOSITORY_BYTES: usize = 100;

/// A validation error that never echoes a submitted repository value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryInputError {
    reason: &'static str,
}

impl RepositoryInputError {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    pub(crate) const fn revision_selector() -> Self {
        Self::new("revision selector is not canonical")
    }

    /// Returns a non-sensitive machine-stable reason.
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for RepositoryInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid public GitHub repository: {}",
            self.reason
        )
    }
}

impl Error for RepositoryInputError {}

/// A canonical public GitHub repository locator.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalGitHubRepository {
    owner: String,
    name: String,
    identifier: String,
    url: String,
}

impl CanonicalGitHubRepository {
    /// Parses an `owner/repository` identifier or an HTTPS GitHub URL.
    pub fn parse(input: &str) -> Result<Self, RepositoryInputError> {
        if input.is_empty() || input.trim() != input || !input.is_ascii() {
            return Err(RepositoryInputError::new("unsupported input syntax"));
        }

        let identifier = if let Some(rest) = input.strip_prefix("https://") {
            let (host, path) = rest
                .split_once('/')
                .ok_or_else(|| RepositoryInputError::new("repository path is missing"))?;
            if !matches!(host, "github.com" | "www.github.com") {
                return Err(RepositoryInputError::new("host is not allowed"));
            }
            path
        } else if input.contains("://") || input.contains('@') || input.contains(':') {
            return Err(RepositoryInputError::new("unsupported input syntax"));
        } else {
            input
        };

        if identifier.contains(['?', '#', '%']) {
            return Err(RepositoryInputError::new("URL decorations are not allowed"));
        }

        let mut identifier = identifier
            .strip_suffix('/')
            .unwrap_or(identifier)
            .to_owned();
        if identifier.to_ascii_lowercase().ends_with(".git") {
            identifier.truncate(identifier.len() - 4);
        }
        let mut components = identifier.split('/');
        let owner = components
            .next()
            .ok_or_else(|| RepositoryInputError::new("owner is missing"))?;
        let name = components
            .next()
            .ok_or_else(|| RepositoryInputError::new("repository is missing"))?;
        if components.next().is_some() {
            return Err(RepositoryInputError::new(
                "extra path components are not allowed",
            ));
        }
        validate_owner(owner)?;
        validate_repository(name)?;

        let owner = owner.to_ascii_lowercase();
        let name = name.to_ascii_lowercase();
        let identifier = format!("{owner}/{name}");
        let url = format!("https://github.com/{identifier}");
        Ok(Self {
            owner,
            name,
            identifier,
            url,
        })
    }

    /// Returns the canonical lowercase owner.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Returns the canonical lowercase repository name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the canonical `owner/repository` identifier.
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the canonical public HTTPS URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}

fn validate_owner(value: &str) -> Result<(), RepositoryInputError> {
    if value.is_empty()
        || value.len() > MAX_OWNER_BYTES
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(RepositoryInputError::new("owner is not canonical"));
    }
    Ok(())
}

fn validate_repository(value: &str) -> Result<(), RepositoryInputError> {
    if value.is_empty()
        || value.len() > MAX_REPOSITORY_BYTES
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(RepositoryInputError::new(
            "repository name is not canonical",
        ));
    }
    Ok(())
}
