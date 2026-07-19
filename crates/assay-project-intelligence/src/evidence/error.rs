use std::{error::Error, fmt};

/// Stable redacted assembly failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceAssemblyErrorKind {
    DuplicateClassification,
    ClassificationSnapshotMismatch,
    MixedClassificationPolicy,
    EvidenceIdGeneration,
    EvidenceIdCollision,
}

/// Redacted assembly failure that never retains a path, source, or object ID.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EvidenceAssemblyError {
    kind: EvidenceAssemblyErrorKind,
}

impl EvidenceAssemblyError {
    pub(crate) const fn new(kind: EvidenceAssemblyErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> EvidenceAssemblyErrorKind {
        self.kind
    }
}

impl fmt::Debug for EvidenceAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceAssemblyError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for EvidenceAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "project evidence assembly failed ({:?})",
            self.kind
        )
    }
}

impl Error for EvidenceAssemblyError {}
