use std::{error::Error, ffi::OsStr, fmt, path::Path, time::Duration};

use assay_domain::{ContentHash, EvidenceStatus, RepositorySource, SourceSnapshot};

/// Explicit resource limits applied to every installed-Git collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionLimits {
    /// Maximum lifetime of one Git child process.
    pub command_timeout: Duration,
    /// Maximum retained stdout for metadata commands.
    pub max_stdout_bytes: usize,
    /// Maximum retained stderr before a child is treated as over limit.
    pub max_stderr_bytes: usize,
    /// Maximum tracked entries accepted from one tree.
    pub max_tree_entries: usize,
    /// Maximum filesystem entries inspected while validating the object store.
    pub max_object_store_entries: usize,
    /// Maximum bytes read and hashed from one blob.
    pub max_object_bytes: u64,
    /// Maximum reachable commits reported by the bounded history scan.
    pub max_history_commits: usize,
    /// Maximum changed entries considered for parent rename detection.
    pub max_rename_candidates: usize,
}

impl Default for CollectionLimits {
    fn default() -> Self {
        Self {
            command_timeout: Duration::from_secs(10),
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
            max_tree_entries: 100_000,
            max_object_store_entries: 1_000_000,
            max_object_bytes: 16 * 1024 * 1024,
            max_history_commits: 100_000,
            max_rename_candidates: 1_000,
        }
    }
}

impl CollectionLimits {
    pub(crate) fn is_valid(self) -> bool {
        !self.command_timeout.is_zero()
            && self.max_stdout_bytes > 0
            && self.max_stderr_bytes > 0
            && self.max_tree_entries > 0
            && self.max_object_store_entries > 0
            && self.max_object_bytes > 0
            && self.max_history_commits > 0
            && self.max_rename_candidates > 0
    }
}

/// Stable collection stage safe for diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStage {
    ConfigureAdapter,
    ProbeCapabilities,
    ValidateObjectStore,
    ResolveRevision,
    ResolveTree,
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

/// Tracked entry mode from an immutable tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryMode {
    Regular,
    Executable,
    SymbolicLink,
    Gitlink,
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

/// Immutable Git object kind referenced by a tracked entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectKind {
    Blob,
    Commit,
}

/// Stable reason why full object metadata is not available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectIssue {
    GitlinkContent,
    SizeLimit,
    MissingOrUnreadable,
    Timeout,
    OutputLimit,
    MalformedMetadata,
}

/// Read-only metadata for one referenced object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectMetadata {
    status: EvidenceStatus,
    size: Option<u64>,
    content_hash: Option<ContentHash>,
    issue: Option<ObjectIssue>,
}

impl ObjectMetadata {
    pub(crate) const fn complete(size: u64, content_hash: ContentHash) -> Self {
        Self {
            status: EvidenceStatus::Complete,
            size: Some(size),
            content_hash: Some(content_hash),
            issue: None,
        }
    }

    pub(crate) const fn limited(size: u64) -> Self {
        Self {
            status: EvidenceStatus::Partial,
            size: Some(size),
            content_hash: None,
            issue: Some(ObjectIssue::SizeLimit),
        }
    }

    pub(crate) const fn unresolved(status: EvidenceStatus, issue: ObjectIssue) -> Self {
        Self {
            status,
            size: None,
            content_hash: None,
            issue: Some(issue),
        }
    }

    /// Returns availability independently from the overall snapshot status.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the object size when it was obtained safely.
    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    /// Returns a SHA-256 digest when the complete bounded object was read.
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        self.content_hash.as_ref()
    }

    /// Returns a stable reason for partial, unavailable, or unsupported data.
    pub const fn issue(&self) -> Option<ObjectIssue> {
        self.issue
    }
}

/// One non-tree entry in the immutable root tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackedEntry {
    path: RepositoryPath,
    mode: EntryMode,
    kind: ObjectKind,
    object_id: GitObjectId,
    content: ObjectMetadata,
}

impl TrackedEntry {
    pub(crate) const fn new(
        path: RepositoryPath,
        mode: EntryMode,
        kind: ObjectKind,
        object_id: GitObjectId,
        content: ObjectMetadata,
    ) -> Self {
        Self {
            path,
            mode,
            kind,
            object_id,
            content,
        }
    }

    /// Returns the byte-exact repository-relative path.
    pub const fn path(&self) -> &RepositoryPath {
        &self.path
    }

    /// Returns the tracked Git mode.
    pub const fn mode(&self) -> EntryMode {
        self.mode
    }

    /// Returns the referenced Git object kind.
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// Returns the referenced immutable Git object ID.
    pub const fn object_id(&self) -> &GitObjectId {
        &self.object_id
    }

    /// Returns bounded content metadata without source bytes.
    pub const fn content(&self) -> &ObjectMetadata {
        &self.content
    }
}

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

/// Installed Git provenance recorded with every collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProvenance {
    adapter_id: &'static str,
    git_version: String,
    object_format: GitObjectFormat,
}

impl GitProvenance {
    pub(crate) fn new(git_version: String, object_format: GitObjectFormat) -> Self {
        Self {
            adapter_id: "installed-git-cli-v1",
            git_version,
            object_format,
        }
    }

    /// Returns the stable adapter identifier.
    pub const fn adapter_id(&self) -> &'static str {
        self.adapter_id
    }

    /// Returns the normalized version reported by the probed executable.
    pub fn git_version(&self) -> &str {
        &self.git_version
    }

    /// Returns the repository object identifier algorithm used by every fact.
    pub const fn object_format(&self) -> GitObjectFormat {
        self.object_format
    }
}

/// Complete usable facts from one immutable repository snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshot {
    source_snapshot: SourceSnapshot,
    status: EvidenceStatus,
    entries: Vec<TrackedEntry>,
    history: HistoryAvailability,
    parent_delta: ParentDelta,
    provenance: GitProvenance,
}

impl RepositorySnapshot {
    pub(crate) const fn new(
        source_snapshot: SourceSnapshot,
        status: EvidenceStatus,
        entries: Vec<TrackedEntry>,
        history: HistoryAvailability,
        parent_delta: ParentDelta,
        provenance: GitProvenance,
    ) -> Self {
        Self {
            source_snapshot,
            status,
            entries,
            history,
            parent_delta,
            provenance,
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
    fn collect(&self, request: SnapshotRequest<'_>) -> Result<RepositorySnapshot, CollectionError>;
}
