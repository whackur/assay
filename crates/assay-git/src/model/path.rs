use std::fmt;

use crate::{CollectionError, CollectionErrorKind, CollectionStage};

/// Byte-exact repository-relative path returned by Git plumbing.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RepositoryPath(Vec<u8>);

impl RepositoryPath {
    pub(crate) fn new(bytes: Vec<u8>) -> Result<Self, CollectionError> {
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        Ok(Self(bytes))
    }

    /// Returns the path exactly as emitted by NUL-delimited Git plumbing.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for RepositoryPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryPath")
            .field("byte_length", &self.0.len())
            .finish()
    }
}
