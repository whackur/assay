use std::str::FromStr;

use assay_domain::{
    AnalysisVersion, ContentHash, DomainValueError, EvidenceId, EvidenceStatus,
    RubricApplicability, RubricCriterionId, RubricJudgment, RubricJudgmentSet,
};
use serde::Serialize;

use crate::{EvidenceScope, ExternalTransmission};

use super::types::{Applicability, EvaluationStatus};

/// Provider judgment accepted against the exact rubric and evidence bundle.
#[derive(Clone, PartialEq, Serialize)]
pub struct ValidatedJudgment {
    pub(crate) criterion_id: String,
    pub(crate) applicability: Applicability,
    pub(crate) rating: Option<u8>,
    pub(crate) rating_scale: u8,
    pub(crate) confidence: f64,
    pub(crate) evidence_ids: Vec<EvidenceId>,
    pub(crate) rationale: String,
}

impl std::fmt::Debug for ValidatedJudgment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedJudgment")
            .field("criterion_id", &self.criterion_id)
            .field("applicability", &self.applicability)
            .field("rating", &self.rating)
            .field("rating_scale", &self.rating_scale)
            .field("confidence", &self.confidence)
            .field("evidence_ids", &self.evidence_ids)
            .field("rationale", &"<provider-prose>")
            .finish()
    }
}

impl ValidatedJudgment {
    pub fn criterion_id(&self) -> &str {
        &self.criterion_id
    }

    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Absent only when not applicable.
    pub const fn rating(&self) -> Option<u8> {
        self.rating
    }

    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }

    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Citations proven to exist in the input bundle.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }

    /// Untrusted provider prose for explanation only, never scoring.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }
}

/// Numeric and citation-only view for deterministic score compilation.
#[derive(Clone, Copy, Debug)]
pub struct ScoringJudgment<'a> {
    criterion_id: &'a str,
    applicability: Applicability,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: &'a [EvidenceId],
}

impl<'a> ScoringJudgment<'a> {
    pub const fn criterion_id(&self) -> &'a str {
        self.criterion_id
    }

    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    pub const fn rating(&self) -> Option<u8> {
        self.rating
    }

    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }

    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    pub const fn evidence_ids(&self) -> &'a [EvidenceId] {
        self.evidence_ids
    }
}

/// Canonical validated result implementing `ai-judgment/v1`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ValidatedJudgmentSet {
    pub(crate) schema_version: String,
    pub(crate) evaluation_version: String,
    pub(crate) rubric_version: String,
    pub(crate) status: EvaluationStatus,
    pub(crate) evidence_bundle_hash: String,
    pub(crate) privacy: ValidatedPrivacy,
    pub(crate) judgments: Vec<ValidatedJudgment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct ValidatedPrivacy {
    pub(crate) evidence_scope: EvidenceScope,
    pub(crate) external_transmission: ExternalTransmission,
}

impl ValidatedJudgmentSet {
    pub(crate) const fn is_usable(&self) -> bool {
        self.status.is_usable()
    }

    pub fn rubric_version(&self) -> &str {
        &self.rubric_version
    }

    pub fn evidence_bundle_hash(&self) -> &str {
        &self.evidence_bundle_hash
    }

    pub fn judgments(&self) -> &[ValidatedJudgment] {
        &self.judgments
    }

    /// Score-compiler view with no access to provider rationale.
    pub fn scoring_judgments(&self) -> impl Iterator<Item = ScoringJudgment<'_>> {
        self.judgments.iter().map(|judgment| ScoringJudgment {
            criterion_id: &judgment.criterion_id,
            applicability: judgment.applicability,
            rating: judgment.rating,
            rating_scale: judgment.rating_scale,
            confidence: judgment.confidence,
            evidence_ids: &judgment.evidence_ids,
        })
    }

    /// Drops provider rationale so no provider prose or score reaches a published score.
    pub fn to_rubric_judgment_set(&self) -> Result<RubricJudgmentSet, DomainValueError> {
        let judgments = self
            .scoring_judgments()
            .map(|judgment| {
                RubricJudgment::new(
                    RubricCriterionId::from_str(judgment.criterion_id())?,
                    map_applicability(judgment.applicability()),
                    judgment.rating(),
                    judgment.rating_scale(),
                    judgment.confidence(),
                    judgment.evidence_ids().to_vec(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        RubricJudgmentSet::new(
            AnalysisVersion::from_str(&self.evaluation_version)?,
            AnalysisVersion::from_str(&self.rubric_version)?,
            map_status(self.status),
            ContentHash::from_str(&self.evidence_bundle_hash)?,
            judgments,
        )
    }
}

pub(crate) const fn map_applicability(applicability: Applicability) -> RubricApplicability {
    match applicability {
        Applicability::Applicable => RubricApplicability::Applicable,
        Applicability::PartiallyApplicable => RubricApplicability::PartiallyApplicable,
        Applicability::NotApplicable => RubricApplicability::NotApplicable,
    }
}

pub(crate) const fn map_status(status: EvaluationStatus) -> EvidenceStatus {
    match status {
        EvaluationStatus::Complete => EvidenceStatus::Complete,
        EvaluationStatus::Partial => EvidenceStatus::Partial,
        EvaluationStatus::Unavailable => EvidenceStatus::Unavailable,
        EvaluationStatus::Unsupported => EvidenceStatus::Unsupported,
        EvaluationStatus::Insufficient => EvidenceStatus::Insufficient,
        EvaluationStatus::Pending => EvidenceStatus::Pending,
    }
}
