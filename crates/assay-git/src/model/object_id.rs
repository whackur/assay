use std::fmt;

use crate::{CollectionError, CollectionErrorKind, CollectionStage};

/// Validated SHA-1 or SHA-256 Git object identifier.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GitObjectId(String);

impl GitObjectId {
    pub(crate) fn parse(
        bytes: &[u8],
        stage: CollectionStage,
        format: GitObjectFormat,
    ) -> Result<Self, CollectionError> {
        if bytes.len() != format.identifier_length()
            || bytes
                .iter()
                .any(|byte| !byte.is_ascii_digit() && !matches!(byte, b'a'..=b'f'))
            || bytes.iter().all(|byte| *byte == b'0')
        {
            return Err(CollectionError::new(
                stage,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        let value = std::str::from_utf8(bytes)
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::MalformedOutput))?;
        Ok(Self(value.to_owned()))
    }

    /// Returns the canonical lowercase hexadecimal identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GitObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitObjectId(<redacted>)")
    }
}

/// Object identifier algorithm declared by the repository.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitObjectFormat {
    Sha1,
    Sha256,
}

impl GitObjectFormat {
    pub(crate) const fn identifier_length(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}
