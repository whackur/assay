use std::str::FromStr;

use crate::cache::error::CacheValueError;

const MAX_VERSION_BYTES: usize = 100;

/// GitHub's stable numeric repository identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderRepositoryId(u64);

impl ProviderRepositoryId {
    /// Creates a non-zero provider repository identifier.
    pub fn new(value: u64) -> Result<Self, CacheValueError> {
        if value == 0 {
            return Err(CacheValueError::new(
                "provider_repository_id",
                "zero is not a provider repository identifier",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the numeric provider repository identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A canonical version or evaluator profile component used in cache keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheVersion(String);

impl CacheVersion {
    /// Parses a lowercase portable cache-key component.
    pub fn parse(value: &str) -> Result<Self, CacheValueError> {
        if value.is_empty()
            || value.len() > MAX_VERSION_BYTES
            || value.contains("..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
            || !value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !value
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(CacheValueError::new(
                "cache_version",
                "expected a canonical lowercase version component",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the canonical value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated GitHub Git object identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GitHubObjectId(String);

impl GitHubObjectId {
    /// Returns the lowercase full object identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitHubObjectId {
    type Err = CacheValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !matches!(value.len(), 40 | 64)
            || value.bytes().all(|byte| byte == b'0')
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(CacheValueError::new(
                "github_object_id",
                "expected a full lowercase non-null Git object identifier",
            ));
        }
        Ok(Self(value.to_owned()))
    }
}
