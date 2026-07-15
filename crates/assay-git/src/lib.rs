//! Git repository history extraction for Assay.
//!
//! Collection implementations depend on domain contracts and must never
//! execute code from an analyzed repository.
//!
//! This crate measures facts pinned to immutable Git objects: the resolved
//! commit and tree IDs, tracked entry metadata, bounded content digests, and
//! bounded history availability. It cannot verify that a project builds,
//! works, is safe, or matches an uncommitted working tree. An unavailable
//! object is missing evidence, not evidence that a file is empty or absent;
//! a content hash is provenance, not a quality or security judgment.

#![forbid(unsafe_code)]

mod cli;
mod model;
mod process;

pub use cli::GitCliAdapter;
pub use model::{
    CollectionError, CollectionErrorKind, CollectionLimits, CollectionStage, EntryMode,
    GitObjectId, GitProvenance, HistoryAvailability, HistoryIssue, ObjectIssue, ObjectKind,
    ObjectMetadata, ParentDelta, ParentDeltaIssue, RepositoryPath, RepositorySnapshot,
    RepositorySnapshotPort, SnapshotRequest, TrackedEntry,
};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
