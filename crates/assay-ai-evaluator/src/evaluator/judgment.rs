use std::str::FromStr;

use assay_domain::{
    AnalysisVersion, ContentHash, DomainValueError, EvidenceId, EvidenceStatus,
    RubricApplicability, RubricCriterionId, RubricJudgment, RubricJudgmentSet,
};
use serde::Serialize;

use crate::{EvidenceScope, ExternalTransmission};

use super::types::{Applicability, EvaluationStatus};

/// A provider judgment accepted against the exact rubric and evidence bundle.
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
    /// Returns the stable project-level criterion ID.
    pub fn criterion_id(&self) -> &str {
        &self.criterion_id
    }

    /// Returns criterion applicability.
    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Returns the bounded rating, absent only when not applicable.
    pub const fn rating(&self) -> Option<u8> {
        self.rating
    }

    /// Returns the inclusive rating upper bound.
    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }

    /// Returns provider confidence after range validation.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns citations proven to exist in the input bundle.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }

    /// Returns bounded untrusted-provider prose for explanation only.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }
}

/// Numeric and citation-only view intended for deterministic score compilation.
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
    /// Returns the stable project criterion ID.
    pub const fn criterion_id(&self) -> &'a str {
        self.criterion_id
    }

    /// Returns applicability without provider prose.
    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Returns the bounded rating.
    pub const fn rating(&self) -> Option<u8> {
        self.rating
    }

    /// Returns the fixed rating scale.
    pub const fn rating_scale(&self) -> u8 {
        self.rating_scale
    }

    /// Returns validated provider confidence.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns validated citations.
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
    /// Returns the rubric version accepted by the validator.
    pub fn rubric_version(&self) -> &str {
        &self.rubric_version
    }

    /// Returns the content hash bound to every accepted citation.
    pub fn evidence_bundle_hash(&self) -> &str {
        &self.evidence_bundle_hash
    }

    /// Returns judgments in canonical criterion order.
    pub fn judgments(&self) -> &[ValidatedJudgment] {
        &self.judgments
    }

    /// Returns a score-compiler view that cannot access provider rationale.
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

    /// Maps the validated result onto the shared domain judgment contract that
    /// the deterministic score compiler consumes.
    ///
    /// Provider rationale is intentionally dropped; only bounded ratings and
    /// citations cross into the compiler contract, so no provider prose or
    /// provider-emitted score can reach a published score.
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
