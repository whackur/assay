use std::fmt;

use crate::evidence::hex::lower_hex;
use assay_classifier::PortablePath;

/// Stable category for a raw repository fact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RawEvidenceKind {
    RepositorySnapshot,
    TrackedFile,
    HistoryScope,
    ParentDelta,
}

impl RawEvidenceKind {
    pub(crate) const fn id_component(self) -> &'static str {
        match self {
            Self::RepositorySnapshot => "repository-snapshot",
            Self::TrackedFile => "tracked-file",
            Self::HistoryScope => "history-scope",
            Self::ParentDelta => "parent-delta",
        }
    }
}

/// Portable representation used when a Git path is not safe UTF-8.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortablePathEncoding {
    Utf8,
    GitPathHex,
}

/// Repository-relative source path with an explicit portable encoding.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortableRepositoryPath {
    pub(crate) encoding: PortablePathEncoding,
    pub(crate) value: String,
}

impl PortableRepositoryPath {
    pub(crate) fn from_git_bytes(bytes: &[u8]) -> Self {
        if let Ok(value) = std::str::from_utf8(bytes)
            && PortablePath::try_from(value).is_ok()
        {
            return Self {
                encoding: PortablePathEncoding::Utf8,
                value: value.to_owned(),
            };
        }
        Self {
            encoding: PortablePathEncoding::GitPathHex,
            value: lower_hex(bytes),
        }
    }

    /// Returns how `value` represents the exact Git path bytes.
    pub const fn encoding(&self) -> PortablePathEncoding {
        self.encoding
    }

    /// Returns a repository-relative UTF-8 path or lowercase exact byte hex.
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for PortableRepositoryPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortableRepositoryPath")
            .field("encoding", &self.encoding)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// Portable Git object-format name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GitObjectFormatRecord {
    Sha1,
    Sha256,
}

/// Stable reason associated with incomplete raw evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RawEvidenceIssue {
    GitlinkContent,
    SizeLimit,
    MissingOrUnreadable,
    Timeout,
    OutputLimit,
    MalformedMetadata,
    HistoryDepthLimit,
    ShallowRepository,
    ProcessFailure,
    MalformedOutput,
    RenameCandidateLimit,
}

/// Stable classification category independent from classifier internals.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClassificationCategoryRecord {
    ProductionCode,
    Test,
    Documentation,
    CiCd,
    Infrastructure,
    SchemaMigration,
    Dependency,
    SecurityPolicy,
    Configuration,
    Generated,
    Vendored,
    BuildOutput,
    Coverage,
    Unknown,
}

/// Stable classification tag independent from classifier internals.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClassificationTagRecord {
    DependencyManifest,
    Lockfile,
    LinguistGenerated,
    LinguistVendored,
    GeneratedSuppressed,
    VendoredSuppressed,
    AttributesUnavailable,
    Minified,
}

/// Stable classification-provenance kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClassificationEvidenceKindRecord {
    PolicyRule,
    LinguistAttribute,
    AttributeFactsUnavailable,
}

/// Explicit reason why usable classification is partial or absent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClassificationAvailabilityReason {
    AttributesUnavailable,
    MissingClassification,
    NonPortablePath,
}
