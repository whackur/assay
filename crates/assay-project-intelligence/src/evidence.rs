use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use assay_classifier::{
    AttributeAvailability, ClassificationCategory, ClassificationDecision,
    ClassificationEvidenceKind, ClassificationPolicy, ClassificationTag, FileClassification,
    FileClassificationInput, LinguistAttributeFacts, PolicyVersion, PortablePath,
    classify_with_policy,
};
use assay_domain::{
    AnalysisStatus, ContentHash, EvidenceId, EvidenceStatus, RepositorySource, RevisionId,
};
use assay_git::{
    EntryMode, GitObjectFormat, HistoryIssue, ObjectIssue, ObjectKind, ParentDeltaIssue,
    RepositorySnapshot, TrackedEntry,
};
use sha2::{Digest, Sha256};

const EVIDENCE_ID_DOMAIN: &[u8] = b"assay.project-intelligence.evidence-id.v1";

/// Stable category for a raw repository fact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RawEvidenceKind {
    RepositorySnapshot,
    TrackedFile,
    HistoryScope,
    ParentDelta,
}

impl RawEvidenceKind {
    const fn id_component(self) -> &'static str {
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
    encoding: PortablePathEncoding,
    value: String,
}

impl PortableRepositoryPath {
    fn from_git_bytes(bytes: &[u8]) -> Self {
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

/// Adapter provenance attached to every source citation.
#[derive(Clone, Eq, PartialEq)]
pub struct GitEvidenceProvenance {
    adapter_id: String,
    git_version: String,
    object_format: GitObjectFormatRecord,
}

impl GitEvidenceProvenance {
    fn from_snapshot(snapshot: &RepositorySnapshot) -> Self {
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
    repository: RepositorySource,
    repository_revision: RevisionId,
    root_tree: Option<RevisionId>,
    path: Option<PortableRepositoryPath>,
    object_id: Option<String>,
    provenance: GitEvidenceProvenance,
}

impl EvidenceSourceRecord {
    fn for_repository(snapshot: &RepositorySnapshot) -> Self {
        Self {
            repository: snapshot.source_snapshot().source().clone(),
            repository_revision: snapshot.source_snapshot().revision().clone(),
            root_tree: snapshot.source_snapshot().root_tree().cloned(),
            path: None,
            object_id: None,
            provenance: GitEvidenceProvenance::from_snapshot(snapshot),
        }
    }

    fn entry(snapshot: &RepositorySnapshot, entry: &TrackedEntry) -> Self {
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

/// Typed raw payload. Fields are exposed through the evidence fact getters;
/// callers cannot construct an inconsistent payload directly.
#[derive(Clone, Eq, PartialEq)]
pub struct RawEvidencePayload(RawEvidencePayloadData);

#[derive(Clone, Eq, PartialEq)]
enum RawEvidencePayloadData {
    RepositorySnapshot,
    TrackedFile(TrackedFilePayload),
    HistoryScope(HistoryScopePayload),
    ParentDelta(ParentDeltaPayload),
}

#[derive(Clone, Eq, PartialEq)]
struct TrackedFilePayload {
    mode: EntryMode,
    object_kind: ObjectKind,
    size_bytes: Option<u64>,
    content_hash: Option<ContentHash>,
    issue: Option<RawEvidenceIssue>,
}

#[derive(Clone, Eq, PartialEq)]
struct HistoryScopePayload {
    reachable_commits: Option<usize>,
    truncated: Option<bool>,
    issue: Option<RawEvidenceIssue>,
}

#[derive(Clone, Eq, PartialEq)]
struct ParentDeltaPayload {
    changed_entries: Option<usize>,
    renames: Option<usize>,
    issue: Option<RawEvidenceIssue>,
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
    fn repository_snapshot() -> Self {
        Self(RawEvidencePayloadData::RepositorySnapshot)
    }

    fn tracked_file(
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

    fn history_scope(
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

    fn parent_delta(
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

/// One immutable raw fact, kept separate from derived classification.
#[derive(Clone, Eq, PartialEq)]
pub struct RawEvidenceFact {
    id: EvidenceId,
    kind: RawEvidenceKind,
    status: EvidenceStatus,
    source: EvidenceSourceRecord,
    payload: RawEvidencePayload,
}

impl RawEvidenceFact {
    /// Returns the versioned, content-derived evidence identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the raw fact kind used for ID domain separation.
    pub const fn kind(&self) -> RawEvidenceKind {
        self.kind
    }

    /// Returns exact source availability without numeric substitution.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns portable immutable citation data.
    pub const fn source(&self) -> &EvidenceSourceRecord {
        &self.source
    }

    /// Returns the typed raw payload.
    pub const fn payload(&self) -> &RawEvidencePayload {
        &self.payload
    }

    /// Returns a complete content digest only when one was collected.
    pub const fn content_hash(&self) -> Option<&ContentHash> {
        match &self.payload.0 {
            RawEvidencePayloadData::TrackedFile(payload) => payload.content_hash.as_ref(),
            _ => None,
        }
    }
}

impl fmt::Debug for RawEvidenceFact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawEvidenceFact")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("source", &self.source)
            .field("payload", &self.payload)
            .finish()
    }
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

/// One canonical rule or resolved-attribute provenance item.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClassificationEvidenceFact {
    kind: ClassificationEvidenceKindRecord,
    rule_id: String,
    attribute_name: Option<&'static str>,
    attribute_value: Option<bool>,
}

impl ClassificationEvidenceFact {
    /// Returns the classification provenance kind.
    pub const fn kind(&self) -> ClassificationEvidenceKindRecord {
        self.kind
    }

    /// Returns the version-scoped rule identifier.
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns a safe well-known attribute name when applicable.
    pub const fn attribute_name(&self) -> Option<&'static str> {
        self.attribute_name
    }

    /// Returns the resolved attribute value when applicable.
    pub const fn attribute_value(&self) -> Option<bool> {
        self.attribute_value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassificationPayload {
    category: ClassificationCategoryRecord,
    tags: Vec<ClassificationTagRecord>,
    rule_id: String,
    confidence_basis_points: u16,
    evidence: Vec<ClassificationEvidenceFact>,
}

/// Explicit reason why usable classification is partial or absent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ClassificationAvailabilityReason {
    AttributesUnavailable,
    MissingClassification,
    NonPortablePath,
}

/// Classification bound to the exact Git path bytes and object ID used to
/// compute it. Construction always invokes a versioned classifier policy.
#[derive(Clone, Eq, PartialEq)]
pub struct ClassifiedSnapshotFile {
    snapshot: SnapshotBinding,
    path_bytes: Vec<u8>,
    object_id: String,
    portable_path: PortableRepositoryPath,
    attempted_policy_version: String,
    status: EvidenceStatus,
    reason: Option<ClassificationAvailabilityReason>,
    payload: Option<ClassificationPayload>,
}

struct PinnedClassificationPolicy<'a, P: ClassificationPolicy + ?Sized> {
    policy: &'a P,
    version: PolicyVersion,
}

impl<P: ClassificationPolicy + ?Sized> ClassificationPolicy for PinnedClassificationPolicy<'_, P> {
    fn policy_version(&self) -> PolicyVersion {
        self.version.clone()
    }

    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision {
        self.policy.evaluate(input)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct SnapshotBinding {
    repository: RepositorySource,
    revision: RevisionId,
    root_tree: Option<RevisionId>,
}

impl SnapshotBinding {
    fn new(snapshot: &RepositorySnapshot) -> Self {
        Self {
            repository: snapshot.source_snapshot().source().clone(),
            revision: snapshot.source_snapshot().revision().clone(),
            root_tree: snapshot.source_snapshot().root_tree().cloned(),
        }
    }

    fn matches(&self, snapshot: &RepositorySnapshot) -> bool {
        self.repository == *snapshot.source_snapshot().source()
            && self.revision == *snapshot.source_snapshot().revision()
            && self.root_tree.as_ref() == snapshot.source_snapshot().root_tree()
    }
}

impl ClassifiedSnapshotFile {
    /// Classifies one immutable snapshot entry without I/O.
    pub fn classify(
        snapshot: &RepositorySnapshot,
        entry: &TrackedEntry,
        attributes: LinguistAttributeFacts,
        policy: &(impl ClassificationPolicy + ?Sized),
    ) -> Result<Self, EvidenceAssemblyError> {
        if !snapshot.entries().iter().any(|candidate| {
            candidate.path().as_bytes() == entry.path().as_bytes()
                && candidate.object_id() == entry.object_id()
        }) {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
            ));
        }
        let path_bytes = entry.path().as_bytes().to_vec();
        let portable_path = PortableRepositoryPath::from_git_bytes(&path_bytes);
        let attempted_policy = policy.policy_version();
        let pinned_policy = PinnedClassificationPolicy {
            policy,
            version: attempted_policy.clone(),
        };
        let classification = std::str::from_utf8(&path_bytes)
            .ok()
            .and_then(|path| FileClassificationInput::try_new(path, attributes).ok())
            .map(|input| classify_with_policy(&pinned_policy, &input));

        let (status, reason, payload) = match classification {
            Some(classification) => {
                let partial =
                    classification.attribute_availability() == AttributeAvailability::Unavailable;
                (
                    if partial {
                        EvidenceStatus::Partial
                    } else {
                        EvidenceStatus::Complete
                    },
                    partial.then_some(ClassificationAvailabilityReason::AttributesUnavailable),
                    Some(map_classification(&classification)),
                )
            }
            None => (
                EvidenceStatus::Unsupported,
                Some(ClassificationAvailabilityReason::NonPortablePath),
                None,
            ),
        };

        Ok(Self {
            snapshot: SnapshotBinding::new(snapshot),
            path_bytes,
            object_id: entry.object_id().as_str().to_owned(),
            portable_path,
            attempted_policy_version: attempted_policy.as_str().to_owned(),
            status,
            reason,
            payload,
        })
    }

    fn key(&self) -> (Vec<u8>, String) {
        (self.path_bytes.clone(), self.object_id.clone())
    }
}

impl fmt::Debug for ClassifiedSnapshotFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClassifiedSnapshotFile")
            .field("snapshot", &"<immutable-snapshot>")
            .field("path", &self.portable_path)
            .field("object_id", &"<immutable-object>")
            .field("status", &self.status)
            .field("reason", &self.reason)
            .field("policy_version", &self.attempted_policy_version)
            .finish()
    }
}

/// One derived classification fact citing exactly one raw tracked-file fact.
#[derive(Clone, Eq, PartialEq)]
pub struct ClassificationEvidenceRecord {
    id: EvidenceId,
    status: EvidenceStatus,
    source_evidence_id: EvidenceId,
    source: EvidenceSourceRecord,
    attempted_policy_version: Option<String>,
    reason: Option<ClassificationAvailabilityReason>,
    payload: Option<ClassificationPayload>,
}

impl ClassificationEvidenceRecord {
    /// Returns the versioned, content-derived classification evidence ID.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns classification availability.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the cited raw file evidence ID.
    pub const fn source_evidence_id(&self) -> Option<&EvidenceId> {
        Some(&self.source_evidence_id)
    }

    /// Returns portable source data matching the cited raw fact.
    pub const fn source(&self) -> &EvidenceSourceRecord {
        &self.source
    }

    /// Returns the explicit partial, missing, or unsupported reason.
    pub const fn reason(&self) -> Option<ClassificationAvailabilityReason> {
        self.reason
    }

    /// Returns the attempted policy for every supplied classification.
    /// Only an automatically generated missing-classification record has none.
    pub fn policy_version(&self) -> Option<&str> {
        self.attempted_policy_version.as_deref()
    }

    /// Returns the primary category only when classification was produced.
    pub fn category(&self) -> Option<ClassificationCategoryRecord> {
        self.payload.as_ref().map(|payload| payload.category)
    }

    /// Returns canonical secondary tags, empty only when unavailable or none.
    pub fn tags(&self) -> &[ClassificationTagRecord] {
        self.payload
            .as_ref()
            .map_or(&[], |payload| payload.tags.as_slice())
    }

    /// Returns the primary rule identifier when classification was produced.
    pub fn rule_id(&self) -> Option<&str> {
        self.payload
            .as_ref()
            .map(|payload| payload.rule_id.as_str())
    }

    /// Returns integer policy confidence, not a project-quality score.
    pub fn confidence_basis_points(&self) -> Option<u16> {
        self.payload
            .as_ref()
            .map(|payload| payload.confidence_basis_points)
    }

    /// Returns canonical rule and attribute provenance.
    pub fn classification_evidence(&self) -> &[ClassificationEvidenceFact] {
        self.payload
            .as_ref()
            .map_or(&[], |payload| payload.evidence.as_slice())
    }
}

impl fmt::Debug for ClassificationEvidenceRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClassificationEvidenceRecord")
            .field("id", &self.id)
            .field("status", &self.status)
            .field("source_evidence_id", &self.source_evidence_id)
            .field("source", &self.source)
            .field("reason", &self.reason)
            .field("policy_version", &self.policy_version())
            .field("category", &self.category())
            .finish()
    }
}

/// A canonical typed project-evidence manifest with no scores or people.
#[derive(Clone, Eq, PartialEq)]
pub struct ProjectEvidenceManifest {
    status: AnalysisStatus,
    classification_policy_version: Option<String>,
    raw_facts: Vec<RawEvidenceFact>,
    classification_facts: Vec<ClassificationEvidenceRecord>,
}

impl ProjectEvidenceManifest {
    /// Returns complete only when collection and every classification are complete.
    pub const fn status(&self) -> AnalysisStatus {
        self.status
    }

    /// Returns the one policy version used by all present classifications.
    pub fn classification_policy_version(&self) -> Option<&str> {
        self.classification_policy_version.as_deref()
    }

    /// Returns raw facts in canonical evidence-ID order.
    pub fn raw_facts(&self) -> &[RawEvidenceFact] {
        &self.raw_facts
    }

    /// Returns classification facts in canonical evidence-ID order.
    pub fn classification_facts(&self) -> &[ClassificationEvidenceRecord] {
        &self.classification_facts
    }

    /// Iterates all IDs without merging raw and derived containers.
    pub fn all_evidence_ids(&self) -> impl Iterator<Item = &EvidenceId> {
        self.raw_facts.iter().map(RawEvidenceFact::id).chain(
            self.classification_facts
                .iter()
                .map(ClassificationEvidenceRecord::id),
        )
    }
}

impl fmt::Debug for ProjectEvidenceManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectEvidenceManifest")
            .field("status", &self.status)
            .field(
                "classification_policy_version",
                &self.classification_policy_version,
            )
            .field("raw_fact_count", &self.raw_facts.len())
            .field(
                "classification_fact_count",
                &self.classification_facts.len(),
            )
            .finish()
    }
}

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
    const fn new(kind: EvidenceAssemblyErrorKind) -> Self {
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

/// Combines one immutable snapshot with zero or more bound classification facts.
///
/// A missing classification becomes an explicit unavailable, citable record.
/// Duplicate and foreign bindings fail closed. All supplied classifications,
/// including unsupported attempts,
/// must use one policy version.
pub fn assemble_project_evidence(
    snapshot: &RepositorySnapshot,
    classifications: impl IntoIterator<Item = ClassifiedSnapshotFile>,
) -> Result<ProjectEvidenceManifest, EvidenceAssemblyError> {
    let mut raw_facts = raw_facts(snapshot)?;
    let raw_files = raw_file_lookup(&raw_facts);
    let snapshot_keys = snapshot
        .entries()
        .iter()
        .map(|entry| (entry_key(entry), ()))
        .collect::<BTreeMap<_, _>>();

    let mut supplied = BTreeMap::new();
    let mut policy_version: Option<String> = None;
    for classification in classifications {
        if !classification.snapshot.matches(snapshot) {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
            ));
        }
        let key = classification.key();
        if !snapshot_keys.contains_key(&key) {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
            ));
        }
        if let Some(expected) = &policy_version {
            if expected != &classification.attempted_policy_version {
                return Err(EvidenceAssemblyError::new(
                    EvidenceAssemblyErrorKind::MixedClassificationPolicy,
                ));
            }
        } else {
            policy_version = Some(classification.attempted_policy_version.clone());
        }
        if supplied.insert(key, classification).is_some() {
            return Err(EvidenceAssemblyError::new(
                EvidenceAssemblyErrorKind::DuplicateClassification,
            ));
        }
    }

    let mut classification_facts = Vec::with_capacity(snapshot.entries().len());
    for entry in snapshot.entries() {
        let key = entry_key(entry);
        let raw = raw_files.get(&key).ok_or_else(|| {
            EvidenceAssemblyError::new(EvidenceAssemblyErrorKind::EvidenceIdCollision)
        })?;
        let source = EvidenceSourceRecord::entry(snapshot, entry);
        let bound = supplied.remove(&key);
        classification_facts.push(classification_fact(raw.id(), source, bound)?);
    }
    if !supplied.is_empty() {
        return Err(EvidenceAssemblyError::new(
            EvidenceAssemblyErrorKind::ClassificationSnapshotMismatch,
        ));
    }

    raw_facts.sort_by(|left, right| left.id.cmp(&right.id));
    classification_facts.sort_by(|left, right| left.id.cmp(&right.id));
    reject_duplicate_ids(
        raw_facts.iter().map(RawEvidenceFact::id).chain(
            classification_facts
                .iter()
                .map(ClassificationEvidenceRecord::id),
        ),
    )?;

    let status = if snapshot.status() == EvidenceStatus::Complete
        && classification_facts
            .iter()
            .all(|fact| fact.status == EvidenceStatus::Complete)
    {
        AnalysisStatus::Complete
    } else {
        AnalysisStatus::Partial
    };

    Ok(ProjectEvidenceManifest {
        status,
        classification_policy_version: policy_version,
        raw_facts,
        classification_facts,
    })
}

fn raw_facts(snapshot: &RepositorySnapshot) -> Result<Vec<RawEvidenceFact>, EvidenceAssemblyError> {
    let repository_source = EvidenceSourceRecord::for_repository(snapshot);
    let mut facts = Vec::with_capacity(snapshot.entries().len() + 3);

    let snapshot_payload = RawEvidencePayload::repository_snapshot();
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::RepositorySnapshot,
        snapshot.status(),
        repository_source.clone(),
        snapshot_payload,
    )?);

    for entry in snapshot.entries() {
        let payload = RawEvidencePayload::tracked_file(
            entry.mode(),
            entry.kind(),
            entry.content().size(),
            entry.content().content_hash().cloned(),
            entry.content().issue().map(map_object_issue),
        );
        facts.push(raw_fact(
            snapshot,
            RawEvidenceKind::TrackedFile,
            entry.content().status(),
            EvidenceSourceRecord::entry(snapshot, entry),
            payload,
        )?);
    }

    let history_usable = matches!(
        snapshot.history().status(),
        EvidenceStatus::Complete | EvidenceStatus::Partial
    );
    let history = RawEvidencePayload::history_scope(
        history_usable.then_some(snapshot.history().reachable_commits()),
        history_usable.then_some(snapshot.history().truncated()),
        snapshot.history().issue().map(map_history_issue),
    );
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::HistoryScope,
        snapshot.history().status(),
        repository_source.clone(),
        history,
    )?);

    let (changed_entries, renames) = parent_delta_values(
        snapshot.parent_delta().status(),
        snapshot.parent_delta().issue(),
        snapshot.parent_delta().changed_entries(),
        snapshot.parent_delta().renames(),
    );
    let delta = RawEvidencePayload::parent_delta(
        changed_entries,
        renames,
        snapshot.parent_delta().issue().map(map_parent_delta_issue),
    );
    facts.push(raw_fact(
        snapshot,
        RawEvidenceKind::ParentDelta,
        snapshot.parent_delta().status(),
        repository_source,
        delta,
    )?);
    Ok(facts)
}

fn raw_fact(
    snapshot: &RepositorySnapshot,
    kind: RawEvidenceKind,
    status: EvidenceStatus,
    source: EvidenceSourceRecord,
    payload: RawEvidencePayload,
) -> Result<RawEvidenceFact, EvidenceAssemblyError> {
    let mut id = EvidenceIdBuilder::new(kind.id_component());
    add_snapshot_scope(&mut id, snapshot);
    id.optional_field(
        b"path_bytes",
        source
            .path
            .as_ref()
            .and_then(portable_path_bytes)
            .as_deref(),
    );
    id.optional_field(b"object_id", source.object_id.as_deref().map(str::as_bytes));
    id.field(b"status", evidence_status_code(status).as_bytes());
    add_raw_payload(&mut id, &payload);
    Ok(RawEvidenceFact {
        id: id.finish(kind.id_component())?,
        kind,
        status,
        source,
        payload,
    })
}

fn classification_fact(
    raw_id: &EvidenceId,
    source: EvidenceSourceRecord,
    classification: Option<ClassifiedSnapshotFile>,
) -> Result<ClassificationEvidenceRecord, EvidenceAssemblyError> {
    let (status, attempted_policy_version, reason, payload) = classification.map_or(
        (
            EvidenceStatus::Unavailable,
            None,
            Some(ClassificationAvailabilityReason::MissingClassification),
            None,
        ),
        |value| {
            (
                value.status,
                Some(value.attempted_policy_version),
                value.reason,
                value.payload,
            )
        },
    );
    let mut id = EvidenceIdBuilder::new("file-classification");
    id.field(b"raw_evidence_id", raw_id.as_str().as_bytes());
    id.field(b"status", evidence_status_code(status).as_bytes());
    id.optional_field(
        b"attempted_policy_version",
        attempted_policy_version.as_deref().map(str::as_bytes),
    );
    id.optional_field(
        b"reason",
        reason.map(classification_reason_code).map(str::as_bytes),
    );
    if let Some(payload) = &payload {
        add_classification_payload(&mut id, payload);
    }
    Ok(ClassificationEvidenceRecord {
        id: id.finish("file-classification")?,
        status,
        source_evidence_id: raw_id.clone(),
        source,
        attempted_policy_version,
        reason,
        payload,
    })
}

fn raw_file_lookup(facts: &[RawEvidenceFact]) -> BTreeMap<(Vec<u8>, String), &RawEvidenceFact> {
    facts
        .iter()
        .filter_map(|fact| {
            if fact.kind != RawEvidenceKind::TrackedFile {
                return None;
            }
            let path = fact.source.path.as_ref()?;
            let bytes = portable_path_bytes(path)?;
            Some(((bytes, fact.source.object_id.clone()?), fact))
        })
        .collect()
}

fn portable_path_bytes(path: &PortableRepositoryPath) -> Option<Vec<u8>> {
    match path.encoding {
        PortablePathEncoding::Utf8 => Some(path.value.as_bytes().to_vec()),
        PortablePathEncoding::GitPathHex => decode_hex(&path.value),
    }
}

fn reject_duplicate_ids<'a>(
    ids: impl IntoIterator<Item = &'a EvidenceId>,
) -> Result<(), EvidenceAssemblyError> {
    let mut values = ids.into_iter().map(EvidenceId::as_str).collect::<Vec<_>>();
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(EvidenceAssemblyError::new(
            EvidenceAssemblyErrorKind::EvidenceIdCollision,
        ));
    }
    Ok(())
}

fn entry_key(entry: &TrackedEntry) -> (Vec<u8>, String) {
    (
        entry.path().as_bytes().to_vec(),
        entry.object_id().as_str().to_owned(),
    )
}

fn map_classification(classification: &FileClassification) -> ClassificationPayload {
    let mut tags = classification
        .tags()
        .iter()
        .copied()
        .map(map_classification_tag)
        .collect::<Vec<_>>();
    tags.sort_unstable();
    tags.dedup();
    let mut evidence = classification
        .evidence()
        .iter()
        .map(|item| ClassificationEvidenceFact {
            kind: match item.kind() {
                ClassificationEvidenceKind::PolicyRule => {
                    ClassificationEvidenceKindRecord::PolicyRule
                }
                ClassificationEvidenceKind::LinguistAttribute => {
                    ClassificationEvidenceKindRecord::LinguistAttribute
                }
                ClassificationEvidenceKind::AttributeFactsUnavailable => {
                    ClassificationEvidenceKindRecord::AttributeFactsUnavailable
                }
            },
            rule_id: item.rule_id().as_str().to_owned(),
            attribute_name: item.attribute_name(),
            attribute_value: item.attribute_value(),
        })
        .collect::<Vec<_>>();
    evidence.sort();
    evidence.dedup();
    ClassificationPayload {
        category: map_classification_category(classification.category()),
        tags,
        rule_id: classification.rule_id().as_str().to_owned(),
        confidence_basis_points: classification.confidence().basis_points(),
        evidence,
    }
}

fn map_classification_category(value: ClassificationCategory) -> ClassificationCategoryRecord {
    match value {
        ClassificationCategory::ProductionCode => ClassificationCategoryRecord::ProductionCode,
        ClassificationCategory::Test => ClassificationCategoryRecord::Test,
        ClassificationCategory::Documentation => ClassificationCategoryRecord::Documentation,
        ClassificationCategory::CiCd => ClassificationCategoryRecord::CiCd,
        ClassificationCategory::Infrastructure => ClassificationCategoryRecord::Infrastructure,
        ClassificationCategory::SchemaMigration => ClassificationCategoryRecord::SchemaMigration,
        ClassificationCategory::Dependency => ClassificationCategoryRecord::Dependency,
        ClassificationCategory::SecurityPolicy => ClassificationCategoryRecord::SecurityPolicy,
        ClassificationCategory::Configuration => ClassificationCategoryRecord::Configuration,
        ClassificationCategory::Generated => ClassificationCategoryRecord::Generated,
        ClassificationCategory::Vendored => ClassificationCategoryRecord::Vendored,
        ClassificationCategory::BuildOutput => ClassificationCategoryRecord::BuildOutput,
        ClassificationCategory::Coverage => ClassificationCategoryRecord::Coverage,
        ClassificationCategory::Unknown => ClassificationCategoryRecord::Unknown,
    }
}

fn map_classification_tag(value: ClassificationTag) -> ClassificationTagRecord {
    match value {
        ClassificationTag::DependencyManifest => ClassificationTagRecord::DependencyManifest,
        ClassificationTag::Lockfile => ClassificationTagRecord::Lockfile,
        ClassificationTag::LinguistGenerated => ClassificationTagRecord::LinguistGenerated,
        ClassificationTag::LinguistVendored => ClassificationTagRecord::LinguistVendored,
        ClassificationTag::GeneratedSuppressed => ClassificationTagRecord::GeneratedSuppressed,
        ClassificationTag::VendoredSuppressed => ClassificationTagRecord::VendoredSuppressed,
        ClassificationTag::AttributesUnavailable => ClassificationTagRecord::AttributesUnavailable,
        ClassificationTag::Minified => ClassificationTagRecord::Minified,
    }
}

fn map_object_issue(value: ObjectIssue) -> RawEvidenceIssue {
    match value {
        ObjectIssue::GitlinkContent => RawEvidenceIssue::GitlinkContent,
        ObjectIssue::SizeLimit => RawEvidenceIssue::SizeLimit,
        ObjectIssue::MissingOrUnreadable => RawEvidenceIssue::MissingOrUnreadable,
        ObjectIssue::Timeout => RawEvidenceIssue::Timeout,
        ObjectIssue::OutputLimit => RawEvidenceIssue::OutputLimit,
        ObjectIssue::MalformedMetadata => RawEvidenceIssue::MalformedMetadata,
    }
}

fn map_history_issue(value: HistoryIssue) -> RawEvidenceIssue {
    match value {
        HistoryIssue::DepthLimit => RawEvidenceIssue::HistoryDepthLimit,
        HistoryIssue::ShallowRepository => RawEvidenceIssue::ShallowRepository,
        HistoryIssue::ProcessFailure => RawEvidenceIssue::ProcessFailure,
        HistoryIssue::MalformedOutput => RawEvidenceIssue::MalformedOutput,
    }
}

fn map_parent_delta_issue(value: ParentDeltaIssue) -> RawEvidenceIssue {
    match value {
        ParentDeltaIssue::RenameCandidateLimit => RawEvidenceIssue::RenameCandidateLimit,
        ParentDeltaIssue::ShallowRepository => RawEvidenceIssue::ShallowRepository,
        ParentDeltaIssue::ProcessFailure => RawEvidenceIssue::ProcessFailure,
        ParentDeltaIssue::MalformedOutput => RawEvidenceIssue::MalformedOutput,
    }
}

fn parent_delta_values(
    status: EvidenceStatus,
    issue: Option<ParentDeltaIssue>,
    changed_entries: usize,
    renames: usize,
) -> (Option<usize>, Option<usize>) {
    match (status, issue) {
        (EvidenceStatus::Complete, None) => (Some(changed_entries), Some(renames)),
        (EvidenceStatus::Partial, Some(ParentDeltaIssue::RenameCandidateLimit)) => {
            (Some(changed_entries), None)
        }
        _ => (None, None),
    }
}

struct EvidenceIdBuilder(Sha256);

impl EvidenceIdBuilder {
    fn new(kind: &str) -> Self {
        let mut hash = Sha256::new();
        update_length_prefixed(&mut hash, EVIDENCE_ID_DOMAIN);
        update_length_prefixed(&mut hash, kind.as_bytes());
        Self(hash)
    }

    fn field(&mut self, name: &[u8], value: &[u8]) {
        update_length_prefixed(&mut self.0, name);
        update_length_prefixed(&mut self.0, value);
    }

    fn optional_field(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.field(name, if value.is_some() { b"some" } else { b"none" });
        if let Some(value) = value {
            self.field(b"value", value);
        }
    }

    fn finish(self, kind: &str) -> Result<EvidenceId, EvidenceAssemblyError> {
        let digest = lower_hex(&self.0.finalize());
        EvidenceId::from_str(&format!("evidence:{kind}:v1-{digest}")).map_err(|_| {
            EvidenceAssemblyError::new(EvidenceAssemblyErrorKind::EvidenceIdGeneration)
        })
    }
}

fn update_length_prefixed(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn add_snapshot_scope(id: &mut EvidenceIdBuilder, snapshot: &RepositorySnapshot) {
    add_repository_source(id, snapshot.source_snapshot().source());
    id.field(
        b"revision",
        snapshot.source_snapshot().revision().as_str().as_bytes(),
    );
    id.optional_field(
        b"root_tree",
        snapshot
            .source_snapshot()
            .root_tree()
            .map(RevisionId::as_str)
            .map(str::as_bytes),
    );
    id.field(
        b"object_format",
        match snapshot.provenance().object_format() {
            GitObjectFormat::Sha1 => b"sha1",
            GitObjectFormat::Sha256 => b"sha256",
        },
    );
}

fn add_repository_source(id: &mut EvidenceIdBuilder, source: &RepositorySource) {
    if let Some(repository_id) = source.local_repository_id() {
        id.field(b"source_kind", b"local");
        id.field(b"repository_id", repository_id.as_str().as_bytes());
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        id.field(b"source_kind", b"hosted");
        id.field(b"provider", provider.as_bytes());
        id.field(b"namespace", namespace.as_bytes());
        id.field(b"repository", repository.as_bytes());
    }
}

fn add_raw_payload(id: &mut EvidenceIdBuilder, payload: &RawEvidencePayload) {
    match &payload.0 {
        RawEvidencePayloadData::RepositorySnapshot => {
            id.field(b"payload", b"repository_snapshot");
        }
        RawEvidencePayloadData::TrackedFile(payload) => {
            id.field(b"payload", b"tracked_file");
            id.field(b"mode", entry_mode_code(payload.mode).as_bytes());
            id.field(
                b"object_kind",
                object_kind_code(payload.object_kind).as_bytes(),
            );
            id.optional_field(
                b"size_bytes",
                payload
                    .size_bytes
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"content_hash",
                payload
                    .content_hash
                    .as_ref()
                    .map(ContentHash::as_str)
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
        RawEvidencePayloadData::HistoryScope(payload) => {
            id.field(b"payload", b"history_scope");
            id.optional_field(
                b"reachable_commits",
                payload
                    .reachable_commits
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"truncated",
                payload.truncated.map(|value| {
                    if value {
                        b"true".as_slice()
                    } else {
                        b"false".as_slice()
                    }
                }),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
        RawEvidencePayloadData::ParentDelta(payload) => {
            id.field(b"payload", b"parent_delta");
            id.optional_field(
                b"changed_entries",
                payload
                    .changed_entries
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"renames",
                payload
                    .renames
                    .as_ref()
                    .map(|value| value.to_string())
                    .as_deref()
                    .map(str::as_bytes),
            );
            id.optional_field(
                b"issue",
                payload.issue.map(raw_issue_code).map(str::as_bytes),
            );
        }
    }
}

fn add_classification_payload(id: &mut EvidenceIdBuilder, payload: &ClassificationPayload) {
    id.field(
        b"category",
        classification_category_code(payload.category).as_bytes(),
    );
    for tag in &payload.tags {
        id.field(b"tag", classification_tag_code(*tag).as_bytes());
    }
    id.field(b"rule_id", payload.rule_id.as_bytes());
    id.field(
        b"confidence_basis_points",
        payload.confidence_basis_points.to_string().as_bytes(),
    );
    for evidence in &payload.evidence {
        id.field(
            b"evidence_kind",
            classification_evidence_kind_code(evidence.kind).as_bytes(),
        );
        id.field(b"evidence_rule_id", evidence.rule_id.as_bytes());
        id.optional_field(
            b"attribute_name",
            evidence.attribute_name.map(str::as_bytes),
        );
        id.optional_field(
            b"attribute_value",
            evidence.attribute_value.map(|value| {
                if value {
                    b"true".as_slice()
                } else {
                    b"false".as_slice()
                }
            }),
        );
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some((high << 4 | low) as u8)
        })
        .collect()
}

const fn evidence_status_code(value: EvidenceStatus) -> &'static str {
    match value {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}

const fn entry_mode_code(value: EntryMode) -> &'static str {
    match value {
        EntryMode::Regular => "regular",
        EntryMode::Executable => "executable",
        EntryMode::SymbolicLink => "symbolic_link",
        EntryMode::Gitlink => "gitlink",
    }
}

const fn object_kind_code(value: ObjectKind) -> &'static str {
    match value {
        ObjectKind::Blob => "blob",
        ObjectKind::Commit => "commit",
    }
}

const fn raw_issue_code(value: RawEvidenceIssue) -> &'static str {
    match value {
        RawEvidenceIssue::GitlinkContent => "gitlink_content",
        RawEvidenceIssue::SizeLimit => "size_limit",
        RawEvidenceIssue::MissingOrUnreadable => "missing_or_unreadable",
        RawEvidenceIssue::Timeout => "timeout",
        RawEvidenceIssue::OutputLimit => "output_limit",
        RawEvidenceIssue::MalformedMetadata => "malformed_metadata",
        RawEvidenceIssue::HistoryDepthLimit => "history_depth_limit",
        RawEvidenceIssue::ShallowRepository => "shallow_repository",
        RawEvidenceIssue::ProcessFailure => "process_failure",
        RawEvidenceIssue::MalformedOutput => "malformed_output",
        RawEvidenceIssue::RenameCandidateLimit => "rename_candidate_limit",
    }
}

const fn classification_reason_code(value: ClassificationAvailabilityReason) -> &'static str {
    match value {
        ClassificationAvailabilityReason::AttributesUnavailable => "attributes_unavailable",
        ClassificationAvailabilityReason::MissingClassification => "missing_classification",
        ClassificationAvailabilityReason::NonPortablePath => "non_portable_path",
    }
}

const fn classification_category_code(value: ClassificationCategoryRecord) -> &'static str {
    match value {
        ClassificationCategoryRecord::ProductionCode => "production_code",
        ClassificationCategoryRecord::Test => "test",
        ClassificationCategoryRecord::Documentation => "documentation",
        ClassificationCategoryRecord::CiCd => "ci_cd",
        ClassificationCategoryRecord::Infrastructure => "infrastructure",
        ClassificationCategoryRecord::SchemaMigration => "schema_migration",
        ClassificationCategoryRecord::Dependency => "dependency",
        ClassificationCategoryRecord::SecurityPolicy => "security_policy",
        ClassificationCategoryRecord::Configuration => "configuration",
        ClassificationCategoryRecord::Generated => "generated",
        ClassificationCategoryRecord::Vendored => "vendored",
        ClassificationCategoryRecord::BuildOutput => "build_output",
        ClassificationCategoryRecord::Coverage => "coverage",
        ClassificationCategoryRecord::Unknown => "unknown",
    }
}

const fn classification_tag_code(value: ClassificationTagRecord) -> &'static str {
    match value {
        ClassificationTagRecord::DependencyManifest => "dependency_manifest",
        ClassificationTagRecord::Lockfile => "lockfile",
        ClassificationTagRecord::LinguistGenerated => "linguist_generated",
        ClassificationTagRecord::LinguistVendored => "linguist_vendored",
        ClassificationTagRecord::GeneratedSuppressed => "generated_suppressed",
        ClassificationTagRecord::VendoredSuppressed => "vendored_suppressed",
        ClassificationTagRecord::AttributesUnavailable => "attributes_unavailable",
        ClassificationTagRecord::Minified => "minified",
    }
}

const fn classification_evidence_kind_code(
    value: ClassificationEvidenceKindRecord,
) -> &'static str {
    match value {
        ClassificationEvidenceKindRecord::PolicyRule => "policy_rule",
        ClassificationEvidenceKindRecord::LinguistAttribute => "linguist_attribute",
        ClassificationEvidenceKindRecord::AttributeFactsUnavailable => {
            "attribute_facts_unavailable"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_id_normalization_is_length_prefixed_and_kind_separated() {
        fn id(kind: &str, fields: &[(&[u8], &[u8])]) -> EvidenceId {
            let mut builder = EvidenceIdBuilder::new(kind);
            for (name, value) in fields {
                builder.field(name, value);
            }
            builder.finish(kind).unwrap()
        }

        let first = id("tracked-file", &[(b"a", b"bc"), (b"ab", b"c")]);
        let ambiguous_without_lengths = id("tracked-file", &[(b"a", b"bca"), (b"b", b"c")]);
        let different_kind = id("file-classification", &[(b"a", b"bc"), (b"ab", b"c")]);

        assert_ne!(first, ambiguous_without_lengths);
        assert_ne!(first, different_kind);
        assert_eq!(
            first.as_str(),
            "evidence:tracked-file:v1-5765416995d5c02f05916bbbb36f2deff800535712caf2397f098db769fc823b"
        );
    }

    #[test]
    fn parent_delta_values_preserve_only_observed_counts() {
        assert_eq!(
            parent_delta_values(EvidenceStatus::Complete, None, 3, 1),
            (Some(3), Some(1))
        );
        assert_eq!(
            parent_delta_values(
                EvidenceStatus::Partial,
                Some(ParentDeltaIssue::RenameCandidateLimit),
                3,
                0,
            ),
            (Some(3), None)
        );
        assert_eq!(
            parent_delta_values(
                EvidenceStatus::Partial,
                Some(ParentDeltaIssue::ShallowRepository),
                0,
                0,
            ),
            (None, None)
        );
        assert_eq!(
            parent_delta_values(
                EvidenceStatus::Unavailable,
                Some(ParentDeltaIssue::ProcessFailure),
                0,
                0,
            ),
            (None, None)
        );
    }
}
