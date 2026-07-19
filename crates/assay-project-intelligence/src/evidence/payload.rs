use std::fmt;

use assay_domain::ContentHash;
use assay_git::{EntryMode, ObjectKind};

use crate::evidence::types::RawEvidenceIssue;

/// Typed raw payload. Fields are exposed through the evidence fact getters;
/// callers cannot construct an inconsistent payload directly.
#[derive(Clone, Eq, PartialEq)]
pub struct RawEvidencePayload(RawEvidencePayloadData);

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum RawEvidencePayloadData {
    RepositorySnapshot,
    TrackedFile(TrackedFilePayload),
    HistoryScope(HistoryScopePayload),
    ParentDelta(ParentDeltaPayload),
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct TrackedFilePayload {
    pub(crate) mode: EntryMode,
    pub(crate) object_kind: ObjectKind,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) content_hash: Option<ContentHash>,
    pub(crate) issue: Option<RawEvidenceIssue>,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct HistoryScopePayload {
    pub(crate) reachable_commits: Option<usize>,
    pub(crate) truncated: Option<bool>,
    pub(crate) issue: Option<RawEvidenceIssue>,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ParentDeltaPayload {
    pub(crate) changed_entries: Option<usize>,
    pub(crate) renames: Option<usize>,
    pub(crate) issue: Option<RawEvidenceIssue>,
}

/// Read-only tracked-file payload view.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TrackedFileEvidence<'a> {
    payload: &'a TrackedFilePayload,
}

impl TrackedFileEvidence<'_> {
    /// Returns the tracked Git mode.
    pub const fn mode(self) -> EntryMode {
        self.payload.mode
    }

    /// Returns the referenced Git object kind.
    pub const fn object_kind(self) -> ObjectKind {
        self.payload.object_kind
    }

    /// Returns object size only when safely observed.
    pub const fn size_bytes(self) -> Option<u64> {
        self.payload.size_bytes
    }

    /// Returns the explicit incomplete-content reason.
    pub const fn issue(self) -> Option<RawEvidenceIssue> {
        self.payload.issue
    }
}

impl<'a> TrackedFileEvidence<'a> {
    /// Returns the complete bounded content digest when available.
    pub const fn content_hash(self) -> Option<&'a ContentHash> {
        self.payload.content_hash.as_ref()
    }
}

/// Read-only history-scope payload view.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct HistoryScopeEvidence<'a> {
    payload: &'a HistoryScopePayload,
}

impl HistoryScopeEvidence<'_> {
    /// Returns the bounded reachable count only when it was observed.
    pub const fn reachable_commits(self) -> Option<usize> {
        self.payload.reachable_commits
    }

    /// Returns truncation only when history evidence was usable.
    pub const fn truncated(self) -> Option<bool> {
        self.payload.truncated
    }

    /// Returns the explicit incomplete-history reason.
    pub const fn issue(self) -> Option<RawEvidenceIssue> {
        self.payload.issue
    }
}

/// Read-only first-parent delta payload view.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ParentDeltaEvidence<'a> {
    payload: &'a ParentDeltaPayload,
}

impl ParentDeltaEvidence<'_> {
    /// Returns raw changed entries only when they were observed.
    pub const fn changed_entries(self) -> Option<usize> {
        self.payload.changed_entries
    }

    /// Returns exact rename count only when rename analysis completed.
    pub const fn renames(self) -> Option<usize> {
        self.payload.renames
    }

    /// Returns the explicit incomplete-delta reason.
    pub const fn issue(self) -> Option<RawEvidenceIssue> {
        self.payload.issue
    }
}

impl RawEvidencePayload {
    pub(crate) fn repository_snapshot() -> Self {
        Self(RawEvidencePayloadData::RepositorySnapshot)
    }

    pub(crate) fn tracked_file(
        mode: EntryMode,
        object_kind: ObjectKind,
        size_bytes: Option<u64>,
        content_hash: Option<ContentHash>,
        issue: Option<RawEvidenceIssue>,
    ) -> Self {
        Self(RawEvidencePayloadData::TrackedFile(TrackedFilePayload {
            mode,
            object_kind,
            size_bytes,
            content_hash,
            issue,
        }))
    }

    pub(crate) fn history_scope(
        reachable_commits: Option<usize>,
        truncated: Option<bool>,
        issue: Option<RawEvidenceIssue>,
    ) -> Self {
        Self(RawEvidencePayloadData::HistoryScope(HistoryScopePayload {
            reachable_commits,
            truncated,
            issue,
        }))
    }

    pub(crate) fn parent_delta(
        changed_entries: Option<usize>,
        renames: Option<usize>,
        issue: Option<RawEvidenceIssue>,
    ) -> Self {
        Self(RawEvidencePayloadData::ParentDelta(ParentDeltaPayload {
            changed_entries,
            renames,
            issue,
        }))
    }

    /// Returns true only for the repository-snapshot payload.
    pub const fn is_repository_snapshot(&self) -> bool {
        matches!(self.0, RawEvidencePayloadData::RepositorySnapshot)
    }

    /// Returns a read-only tracked-file view for that payload kind.
    pub const fn as_tracked_file(&self) -> Option<TrackedFileEvidence<'_>> {
        match &self.0 {
            RawEvidencePayloadData::TrackedFile(payload) => Some(TrackedFileEvidence { payload }),
            _ => None,
        }
    }

    /// Returns a read-only history-scope view for that payload kind.
    pub const fn as_history_scope(&self) -> Option<HistoryScopeEvidence<'_>> {
        match &self.0 {
            RawEvidencePayloadData::HistoryScope(payload) => Some(HistoryScopeEvidence { payload }),
            _ => None,
        }
    }

    /// Returns a read-only first-parent delta view for that payload kind.
    pub const fn as_parent_delta(&self) -> Option<ParentDeltaEvidence<'_>> {
        match &self.0 {
            RawEvidencePayloadData::ParentDelta(payload) => Some(ParentDeltaEvidence { payload }),
            _ => None,
        }
    }

    pub(crate) const fn data(&self) -> &RawEvidencePayloadData {
        &self.0
    }
}

impl fmt::Debug for RawEvidencePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            RawEvidencePayloadData::RepositorySnapshot => formatter.write_str("RepositorySnapshot"),
            RawEvidencePayloadData::TrackedFile(payload) => formatter
                .debug_struct("TrackedFile")
                .field("mode", &payload.mode)
                .field("object_kind", &payload.object_kind)
                .field("size_bytes", &payload.size_bytes)
                .field(
                    "content_hash",
                    &payload.content_hash.as_ref().map(|_| "<digest>"),
                )
                .field("issue", &payload.issue)
                .finish(),
            RawEvidencePayloadData::HistoryScope(payload) => formatter
                .debug_struct("HistoryScope")
                .field("reachable_commits", &payload.reachable_commits)
                .field("truncated", &payload.truncated)
                .field("issue", &payload.issue)
                .finish(),
            RawEvidencePayloadData::ParentDelta(payload) => formatter
                .debug_struct("ParentDelta")
                .field("changed_entries", &payload.changed_entries)
                .field("renames", &payload.renames)
                .field("issue", &payload.issue)
                .finish(),
        }
    }
}
