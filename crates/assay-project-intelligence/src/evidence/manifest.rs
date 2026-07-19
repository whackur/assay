use std::fmt;

use assay_domain::AnalysisStatus;

use crate::evidence::classification_record::ClassificationEvidenceRecord;
use crate::evidence::raw_fact::RawEvidenceFact;
use assay_domain::EvidenceId;

/// A canonical typed project-evidence manifest with no scores or people.
#[derive(Clone, Eq, PartialEq)]
pub struct ProjectEvidenceManifest {
    pub(crate) status: AnalysisStatus,
    pub(crate) classification_policy_version: Option<String>,
    pub(crate) raw_facts: Vec<RawEvidenceFact>,
    pub(crate) classification_facts: Vec<ClassificationEvidenceRecord>,
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
