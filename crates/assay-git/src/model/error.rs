use std::{error::Error, fmt};

use assay_domain::EvidenceStatus;

/// Stable collection stage safe for diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStage {
    ConfigureAdapter,
    ProbeCapabilities,
    ValidateObjectStore,
    ResolveRevision,
    ResolveTree,
    ReadCommitTime,
    DeriveRepositoryIdentity,
    EnumerateTree,
    ReadObjectMetadata,
    HashObject,
    ReadHistory,
    ReadParentDelta,
}

/// Stable failure category that never contains command output or paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionErrorKind {
    InvalidLimits,
    UntrustedExecutable,
    ExecutableMissing,
    PermissionDenied,
    IncompatibleGit,
    ExternalObjectStore,
    RepositoryRedirect,
    NonZeroExit,
    Timeout,
    OutputLimit,
    RecordLimit,
    MalformedOutput,
    Io,
}

/// Redacted adapter failure containing only a stage and safe category.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CollectionError {
    stage: CollectionStage,
    kind: CollectionErrorKind,
}

impl CollectionError {
    pub(crate) const fn new(stage: CollectionStage, kind: CollectionErrorKind) -> Self {
        Self { stage, kind }
    }

    /// Returns the failed collection stage.
    pub const fn stage(&self) -> CollectionStage {
        self.stage
    }

    /// Returns the stable failure category.
    pub const fn kind(&self) -> CollectionErrorKind {
        self.kind
    }

    /// Collection failures represent unavailable evidence, never an empty or
    /// zero-valued fact.
    pub const fn evidence_status(&self) -> EvidenceStatus {
        EvidenceStatus::Unavailable
    }
}

impl fmt::Debug for CollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionError")
            .field("stage", &self.stage)
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for CollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Git collection failed at {:?} ({:?})",
            self.stage, self.kind
        )
    }
}

impl Error for CollectionError {}
