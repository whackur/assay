use std::fmt;

use assay_domain::{RepositorySource, RevisionId};
use assay_git::{GitObjectFormat, RepositorySnapshot, TrackedEntry};

use crate::evidence::types::{GitObjectFormatRecord, PortableRepositoryPath};

/// Adapter provenance attached to every source citation.
#[derive(Clone, Eq, PartialEq)]
pub struct GitEvidenceProvenance {
    adapter_id: String,
    git_version: String,
    object_format: GitObjectFormatRecord,
}

impl GitEvidenceProvenance {
    pub(crate) fn from_snapshot(snapshot: &RepositorySnapshot) -> Self {
        Self {
            adapter_id: snapshot.provenance().adapter_id().to_owned(),
            git_version: snapshot.provenance().git_version().to_owned(),
            object_format: match snapshot.provenance().object_format() {
                GitObjectFormat::Sha1 => GitObjectFormatRecord::Sha1,
                GitObjectFormat::Sha256 => GitObjectFormatRecord::Sha256,
            },
        }
    }

    /// Returns the stable adapter implementation identity.
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }

    /// Returns the sanitized installed Git version recorded by the adapter.
    pub fn git_version(&self) -> &str {
        &self.git_version
    }

    /// Returns the repository-wide object identifier format.
    pub const fn object_format(&self) -> GitObjectFormatRecord {
        self.object_format
    }
}

impl fmt::Debug for GitEvidenceProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitEvidenceProvenance")
            .field("adapter_id", &self.adapter_id)
            .field("git_version", &self.git_version)
            .field("object_format", &self.object_format)
            .finish()
    }
}

/// Immutable, portable source locator sufficient for later citations.
#[derive(Clone, Eq, PartialEq)]
pub struct EvidenceSourceRecord {
    pub(crate) repository: RepositorySource,
    pub(crate) repository_revision: RevisionId,
    pub(crate) root_tree: Option<RevisionId>,
    pub(crate) path: Option<PortableRepositoryPath>,
    pub(crate) object_id: Option<String>,
    pub(crate) provenance: GitEvidenceProvenance,
}

impl EvidenceSourceRecord {
    pub(crate) fn for_repository(snapshot: &RepositorySnapshot) -> Self {
        Self {
            repository: snapshot.source_snapshot().source().clone(),
            repository_revision: snapshot.source_snapshot().revision().clone(),
            root_tree: snapshot.source_snapshot().root_tree().cloned(),
            path: None,
            object_id: None,
            provenance: GitEvidenceProvenance::from_snapshot(snapshot),
        }
    }

    pub(crate) fn entry(snapshot: &RepositorySnapshot, entry: &TrackedEntry) -> Self {
        let mut source = Self::for_repository(snapshot);
        source.path = Some(PortableRepositoryPath::from_git_bytes(
            entry.path().as_bytes(),
        ));
        source.object_id = Some(entry.object_id().as_str().to_owned());
        source
    }

    /// Returns a path-free local identifier or canonical hosted locator.
    pub const fn repository(&self) -> &RepositorySource {
        &self.repository
    }

    /// Returns the exact immutable repository revision.
    pub const fn repository_revision(&self) -> &RevisionId {
        &self.repository_revision
    }

    /// Returns the exact root tree when supplied by Git.
    pub const fn root_tree(&self) -> Option<&RevisionId> {
        self.root_tree.as_ref()
    }

    /// Returns the portable repository-relative path for file evidence.
    pub const fn path(&self) -> Option<&PortableRepositoryPath> {
        self.path.as_ref()
    }

    /// Returns the immutable object ID for file evidence.
    pub fn object_id(&self) -> Option<&str> {
        self.object_id.as_deref()
    }

    /// Returns collector provenance without a local executable path.
    pub const fn provenance(&self) -> &GitEvidenceProvenance {
        &self.provenance
    }
}

impl fmt::Debug for EvidenceSourceRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceSourceRecord")
            .field("repository", &"<portable-source>")
            .field("repository_revision", &"<immutable-revision>")
            .field(
                "root_tree",
                &self.root_tree.as_ref().map(|_| "<immutable-tree>"),
            )
            .field("path", &self.path)
            .field(
                "object_id",
                &self.object_id.as_ref().map(|_| "<immutable-object>"),
            )
            .field("provenance", &self.provenance)
            .finish()
    }
}
