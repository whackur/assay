use assay_domain::EvidenceStatus;

/// Stable reason why bounded history is not complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryIssue {
    DepthLimit,
    ShallowRepository,
    ProcessFailure,
    MalformedOutput,
}

/// Availability and bounded count of commits reachable from the snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryAvailability {
    status: EvidenceStatus,
    reachable_commits: usize,
    truncated: bool,
    issue: Option<HistoryIssue>,
}

impl HistoryAvailability {
    pub(crate) const fn new(
        status: EvidenceStatus,
        reachable_commits: usize,
        truncated: bool,
        issue: Option<HistoryIssue>,
    ) -> Self {
        Self {
            status,
            reachable_commits,
            truncated,
            issue,
        }
    }

    /// Returns history evidence availability.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the number of commits observed within the configured bound.
    pub const fn reachable_commits(&self) -> usize {
        self.reachable_commits
    }

    /// Returns whether more history exists beyond the reported count.
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Returns a stable reason when history is not complete.
    pub const fn issue(&self) -> Option<HistoryIssue> {
        self.issue
    }
}

/// Stable reason why parent-delta evidence is not complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentDeltaIssue {
    RenameCandidateLimit,
    ShallowRepository,
    ProcessFailure,
    MalformedOutput,
}

/// Bounded first-parent change and rename facts for the resolved commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParentDelta {
    status: EvidenceStatus,
    changed_entries: usize,
    renames: usize,
    issue: Option<ParentDeltaIssue>,
}

impl ParentDelta {
    pub(crate) const fn new(
        status: EvidenceStatus,
        changed_entries: usize,
        renames: usize,
        issue: Option<ParentDeltaIssue>,
    ) -> Self {
        Self {
            status,
            changed_entries,
            renames,
            issue,
        }
    }

    /// Returns first-parent delta availability.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the number of observed raw changed entries.
    pub const fn changed_entries(&self) -> usize {
        self.changed_entries
    }

    /// Returns exact renames detected within the configured bound.
    pub const fn renames(&self) -> usize {
        self.renames
    }

    /// Returns a stable reason when delta evidence is not complete.
    pub const fn issue(&self) -> Option<ParentDeltaIssue> {
        self.issue
    }
}
