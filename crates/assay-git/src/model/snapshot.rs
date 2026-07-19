use std::{ffi::OsStr, path::Path};

use assay_domain::{EvidenceStatus, RepositorySource, RevisionId, SourceSnapshot};

use crate::{GitProvenance, HistoryAvailability, ParentDelta, TrackedEntry};

/// Complete usable facts from one immutable repository snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshot {
    source_snapshot: SourceSnapshot,
    status: EvidenceStatus,
    entries: Vec<TrackedEntry>,
    history: HistoryAvailability,
    parent_delta: ParentDelta,
    provenance: GitProvenance,
    commit_time: String,
}

impl RepositorySnapshot {
    pub(crate) const fn new(
        source_snapshot: SourceSnapshot,
        status: EvidenceStatus,
        entries: Vec<TrackedEntry>,
        history: HistoryAvailability,
        parent_delta: ParentDelta,
        provenance: GitProvenance,
        commit_time: String,
    ) -> Self {
        Self {
            source_snapshot,
            status,
            entries,
            history,
            parent_delta,
            provenance,
            commit_time,
        }
    }

    /// Returns the portable domain snapshot pinned to full object IDs.
    pub const fn source_snapshot(&self) -> &SourceSnapshot {
        &self.source_snapshot
    }

    /// Returns complete or explicit partial collection status.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns tracked entries in Git tree byte order.
    pub fn entries(&self) -> &[TrackedEntry] {
        &self.entries
    }

    /// Returns bounded history facts.
    pub const fn history(&self) -> &HistoryAvailability {
        &self.history
    }

    /// Returns bounded first-parent change and rename facts.
    pub const fn parent_delta(&self) -> &ParentDelta {
        &self.parent_delta
    }

    /// Returns adapter and installed Git provenance.
    pub const fn provenance(&self) -> &GitProvenance {
        &self.provenance
    }

    /// Returns the immutable commit timestamp normalized to RFC 3339 UTC.
    pub fn commit_time(&self) -> &str {
        &self.commit_time
    }
}

/// Portable local identity paired with the exact revision used to derive it.
/// Callers must collect this full revision rather than resolving a movable ref
/// a second time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocalRepository {
    source: RepositorySource,
    revision: RevisionId,
}

impl ResolvedLocalRepository {
    pub(crate) const fn new(source: RepositorySource, revision: RevisionId) -> Self {
        Self { source, revision }
    }

    /// Returns the content-derived path-independent source identity.
    pub const fn source(&self) -> &RepositorySource {
        &self.source
    }

    /// Returns the exact commit used for reachable-root identity derivation.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }
}

/// Domain-facing request for one immutable source snapshot.
pub struct SnapshotRequest<'a> {
    repository: &'a Path,
    source: RepositorySource,
    revision: &'a OsStr,
}

impl<'a> SnapshotRequest<'a> {
    /// Creates a request. The source is portable; the local path is never
    /// included in returned facts or diagnostics.
    pub const fn new(repository: &'a Path, source: RepositorySource, revision: &'a OsStr) -> Self {
        Self {
            repository,
            source,
            revision,
        }
    }

    pub(crate) const fn repository(&self) -> &'a Path {
        self.repository
    }

    pub(crate) const fn source(&self) -> &RepositorySource {
        &self.source
    }

    pub(crate) const fn revision(&self) -> &'a OsStr {
        self.revision
    }
}

/// Replaceable port for collecting immutable repository snapshot facts.
pub trait RepositorySnapshotPort {
    /// Resolves and collects one snapshot without executing repository code.
    fn collect(
        &self,
        request: SnapshotRequest<'_>,
    ) -> Result<RepositorySnapshot, crate::CollectionError>;
}
