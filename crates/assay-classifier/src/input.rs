//! Validated facts consumed by a classification policy.

use std::fmt;

use crate::{attributes::LinguistAttributeFacts, path::PortablePath};

/// Validated facts consumed by a classification policy.
pub struct FileClassificationInput {
    path: PortablePath,
    attributes: LinguistAttributeFacts,
}

impl FileClassificationInput {
    /// Validates a repository-relative path and combines it with already
    /// resolved Git attribute facts.
    pub fn try_new(
        path: impl TryInto<PortablePath, Error = crate::ClassificationError>,
        attributes: LinguistAttributeFacts,
    ) -> Result<Self, crate::ClassificationError> {
        Ok(Self {
            path: path.try_into()?,
            attributes,
        })
    }

    /// Returns the portable path for policy evaluation.
    pub const fn path(&self) -> &PortablePath {
        &self.path
    }

    /// Returns resolved Linguist attribute facts.
    pub const fn attributes(&self) -> LinguistAttributeFacts {
        self.attributes
    }
}

impl fmt::Debug for FileClassificationInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileClassificationInput")
            .field("path", &self.path)
            .field("attributes", &self.attributes)
            .finish()
    }
}
