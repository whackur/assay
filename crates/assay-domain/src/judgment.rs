use serde::{Deserialize, Deserializer, Serialize, de};

use crate::error::DomainValueError;
use crate::identifiers::EvidenceId;
use crate::judgment_applicability::RubricApplicability;
use crate::judgment_criterion::RubricCriterionId;

/// One validated qualitative rubric judgment consumed by the score compiler.
///
/// This is the provider-independent contract the deterministic compiler reads.
/// It carries bounded ratings and citations only; it can never carry a final
/// dimension or overall score, so a provider cannot emit or override a published
/// score through it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RubricJudgment {
    criterion_id: RubricCriterionId,
    applicability: RubricApplicability,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl RubricJudgment {
    /// Validates one bounded, cited rubric judgment.
    ///
    /// A `NotApplicable` criterion carries no rating and may cite no evidence.
    /// Every other applicability requires a rating within the inclusive scale
    /// and at least one citation.
    pub fn new(
        criterion_id: RubricCriterionId,
        applicability: RubricApplicability,
        rating: Option<u8>,
        rating_scale: u8,
        confidence: f64,
        mut evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, DomainValueError> {
        if rating_scale == 0 {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "rating scale must be a positive inclusive bound",
            ));
        }
        match (applicability, rating) {
            (RubricApplicability::NotApplicable, Some(_)) => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "a not_applicable criterion must not carry a rating",
                ));
            }
            (RubricApplicability::NotApplicable, None) => {}
            (_, None) => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "an applicable criterion requires a rating",
                ));
            }
            (_, Some(value)) if value > rating_scale => {
                return Err(DomainValueError::new(
                    "rubric_judgment",
                    "a rating must not exceed the inclusive rating scale",
                ));
            }
            (_, Some(_)) => {}
        }
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "confidence must be a finite value in the closed unit interval",
            ));
        }
        if applicability != RubricApplicability::NotApplicable && evidence_ids.is_empty() {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "an applicable criterion requires at least one cited evidence identifier",
            ));
        }
        evidence_ids.sort();
        if evidence_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValueError::new(
                "rubric_judgment",
                "cited evidence identifiers must be unique",
            ));
        }
        Ok(Self {
            criterion_id,
            applicability,
            rating,
            rating_scale,
            confidence,
            evidence_ids,
        })
    }

    /// Returns the stable dotted criterion identifier.
    pub const fn criterion_id(&self) -> &RubricCriterionId {
        &self.criterion_id
    }

    /// Returns criterion applicability without inventing a zero score.
    pub const fn applicability(&self) -> RubricApplicability {
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

    /// Returns validated provider confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns cited evidence identifiers in canonical order.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RubricJudgmentData {
    criterion_id: RubricCriterionId,
    applicability: RubricApplicability,
    rating: Option<u8>,
    rating_scale: u8,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl<'de> Deserialize<'de> for RubricJudgment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = RubricJudgmentData::deserialize(deserializer)?;
        Self::new(
            data.criterion_id,
            data.applicability,
            data.rating,
            data.rating_scale,
            data.confidence,
            data.evidence_ids,
        )
        .map_err(de::Error::custom)
    }
}
