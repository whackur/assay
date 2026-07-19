use assay_domain::{ContentHash, EvidenceStatus};

/// Tracked entry mode from an immutable tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryMode {
    Regular,
    Executable,
    SymbolicLink,
    Gitlink,
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
    path: crate::RepositoryPath,
    mode: EntryMode,
    kind: ObjectKind,
    object_id: crate::GitObjectId,
    content: ObjectMetadata,
}

impl TrackedEntry {
    pub(crate) const fn new(
        path: crate::RepositoryPath,
        mode: EntryMode,
        kind: ObjectKind,
        object_id: crate::GitObjectId,
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
    pub const fn path(&self) -> &crate::RepositoryPath {
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
    pub const fn object_id(&self) -> &crate::GitObjectId {
        &self.object_id
    }

    /// Returns bounded content metadata without source bytes.
    pub const fn content(&self) -> &ObjectMetadata {
        &self.content
    }
}
