use std::fmt;

/// A stable, redacted classification failure that never echoes source material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassificationError {
    pub(crate) kind: ClassificationErrorKind,
}

/// Stable classification failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassificationErrorKind {
    UncitedObservation,
}

impl ClassificationError {
    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> ClassificationErrorKind {
        self.kind
    }
}

impl fmt::Display for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "classification failed ({:?})", self.kind)
    }
}

impl std::error::Error for ClassificationError {}
