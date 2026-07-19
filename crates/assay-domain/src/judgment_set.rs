use serde::Serialize;

use crate::AnalysisVersion;
use crate::ContentHash;
use crate::error::DomainValueError;
use crate::judgment::RubricJudgment;
use crate::status::EvidenceStatus;

/// A validated set of rubric judgments bound to one evidence bundle.
///
/// The compiler consumes this contract from any provider adapter. The bundle
/// hash records which exact bounded evidence produced the judgments.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RubricJudgmentSet {
    evaluation_version: AnalysisVersion,
    rubric_version: AnalysisVersion,
    status: EvidenceStatus,
    evidence_bundle_hash: ContentHash,
    judgments: Vec<RubricJudgment>,
}

impl RubricJudgmentSet {
    /// Validates and canonicalizes one rubric judgment set.
    ///
    /// A usable status carries judgments; a non-usable status carries none.
    /// Criteria are unique and ordered so compilation is deterministic.
    pub fn new(
        evaluation_version: AnalysisVersion,
        rubric_version: AnalysisVersion,
        status: EvidenceStatus,
        evidence_bundle_hash: ContentHash,
        mut judgments: Vec<RubricJudgment>,
    ) -> Result<Self, DomainValueError> {
        let usable = matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial);
        if usable == judgments.is_empty() {
            return Err(DomainValueError::new(
                "rubric_judgment_set",
                "a complete or partial set carries judgments and any other status carries none",
            ));
        }
        judgments.sort_by(|left, right| left.criterion_id().cmp(right.criterion_id()));
        if judgments
            .windows(2)
            .any(|pair| pair[0].criterion_id() == pair[1].criterion_id())
        {
            return Err(DomainValueError::new(
                "rubric_judgment_set",
                "each criterion may be judged at most once",
            ));
        }
        Ok(Self {
            evaluation_version,
            rubric_version,
            status,
            evidence_bundle_hash,
            judgments,
        })
    }

    /// Returns the Project Intelligence evaluation version boundary.
    pub const fn evaluation_version(&self) -> &AnalysisVersion {
        &self.evaluation_version
    }

    /// Returns the rubric version that produced the judgments.
    pub const fn rubric_version(&self) -> &AnalysisVersion {
        &self.rubric_version
    }

    /// Returns the availability of the judgment set as a whole.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the content hash of the evidence bundle bound to every citation.
    pub const fn evidence_bundle_hash(&self) -> &ContentHash {
        &self.evidence_bundle_hash
    }

    /// Returns judgments in canonical criterion order.
    pub fn judgments(&self) -> &[RubricJudgment] {
        &self.judgments
    }
}
