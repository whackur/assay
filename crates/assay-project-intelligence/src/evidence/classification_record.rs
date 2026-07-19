use std::fmt;

use assay_classifier::{
    AttributeAvailability, ClassificationDecision, ClassificationPolicy, FileClassificationInput,
    LinguistAttributeFacts, PolicyVersion, classify_with_policy,
};
use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};
use assay_git::{RepositorySnapshot, TrackedEntry};

use crate::evidence::error::{EvidenceAssemblyError, EvidenceAssemblyErrorKind};
use crate::evidence::source::EvidenceSourceRecord;
use crate::evidence::types::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord,
    ClassificationEvidenceKindRecord, ClassificationTagRecord, PortableRepositoryPath,
};

/// One canonical rule or resolved-attribute provenance item.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClassificationEvidenceFact {
    pub(crate) kind: ClassificationEvidenceKindRecord,
    pub(crate) rule_id: String,
    pub(crate) attribute_name: Option<&'static str>,
    pub(crate) attribute_value: Option<bool>,
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
pub(crate) struct ClassificationPayload {
    pub(crate) category: ClassificationCategoryRecord,
    pub(crate) tags: Vec<ClassificationTagRecord>,
    pub(crate) rule_id: String,
    pub(crate) confidence_basis_points: u16,
    pub(crate) evidence: Vec<ClassificationEvidenceFact>,
}

/// Classification bound to the exact Git path bytes and object ID used to
/// compute it. Construction always invokes a versioned classifier policy.
#[derive(Clone, Eq, PartialEq)]
pub struct ClassifiedSnapshotFile {
    pub(crate) snapshot: SnapshotBinding,
    pub(crate) path_bytes: Vec<u8>,
    pub(crate) object_id: String,
    pub(crate) portable_path: PortableRepositoryPath,
    pub(crate) attempted_policy_version: String,
    pub(crate) status: EvidenceStatus,
    pub(crate) reason: Option<ClassificationAvailabilityReason>,
    pub(crate) payload: Option<ClassificationPayload>,
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
pub(crate) struct SnapshotBinding {
    repository: RepositorySource,
    revision: RevisionId,
    root_tree: Option<RevisionId>,
}

impl SnapshotBinding {
    pub(crate) fn new(snapshot: &RepositorySnapshot) -> Self {
        Self {
            repository: snapshot.source_snapshot().source().clone(),
            revision: snapshot.source_snapshot().revision().clone(),
            root_tree: snapshot.source_snapshot().root_tree().cloned(),
        }
    }

    pub(crate) fn matches(&self, snapshot: &RepositorySnapshot) -> bool {
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
                    Some(crate::evidence::mapping::map_classification(
                        &classification,
                    )),
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

    pub(crate) fn key(&self) -> (Vec<u8>, String) {
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
    pub(crate) id: EvidenceId,
    pub(crate) status: EvidenceStatus,
    pub(crate) source_evidence_id: EvidenceId,
    pub(crate) source: EvidenceSourceRecord,
    pub(crate) attempted_policy_version: Option<String>,
    pub(crate) reason: Option<ClassificationAvailabilityReason>,
    pub(crate) payload: Option<ClassificationPayload>,
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
