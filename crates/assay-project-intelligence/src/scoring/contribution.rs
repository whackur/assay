use assay_domain::{EvidenceId, RubricApplicability, RubricCriterionId};

use crate::scoring::dimensions::ScoreDimension;
use crate::scoring::error::{ScoreCompileError, ScoreCompileErrorKind};
use crate::scoring::validation::{is_machine_code, sorted_unique, validate_normalized};

/// One deterministic rule contribution to a single dimension.
///
/// The optional value is a normalized `0.0..=1.0` sub-score and is present for
/// every applicability except `not_applicable`, which is an explicit exclusion
/// rather than a zero contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct DeterministicContribution {
    rule_id: String,
    dimension: ScoreDimension,
    applicability: RubricApplicability,
    value: Option<f64>,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl DeterministicContribution {
    /// Validates one deterministic rule contribution.
    pub fn new(
        rule_id: &str,
        dimension: ScoreDimension,
        applicability: RubricApplicability,
        value: Option<f64>,
        confidence: f64,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ScoreCompileError> {
        if !is_machine_code(rule_id) {
            return Err(ScoreCompileError::new(
                ScoreCompileErrorKind::InvalidContribution,
            ));
        }
        validate_normalized(applicability, value, confidence, &evidence_ids)
            .map_err(|()| ScoreCompileError::new(ScoreCompileErrorKind::InvalidContribution))?;
        Ok(Self {
            rule_id: rule_id.to_owned(),
            dimension,
            applicability,
            value,
            confidence,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    /// Returns the versioned rule identifier.
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the dimension this contribution scores.
    pub const fn dimension(&self) -> ScoreDimension {
        self.dimension
    }

    pub(crate) fn applicability(&self) -> RubricApplicability {
        self.applicability
    }

    pub(crate) fn value(&self) -> Option<f64> {
        self.value
    }

    pub(crate) fn confidence(&self) -> f64 {
        self.confidence
    }

    pub(crate) fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}

/// Where one score contribution originated, for rule-and-evidence explainability.
#[derive(Clone, Debug, PartialEq)]
pub enum ContributionSource {
    DeterministicRule(String),
    RubricCriterion(RubricCriterionId),
}

/// One explainable contribution to a dimension score.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreContribution {
    source: ContributionSource,
    applicability: RubricApplicability,
    normalized_value: Option<f64>,
    confidence: f64,
    evidence_ids: Vec<EvidenceId>,
}

impl ScoreContribution {
    pub(crate) fn new(
        source: ContributionSource,
        applicability: RubricApplicability,
        normalized_value: Option<f64>,
        confidence: f64,
        evidence_ids: Vec<EvidenceId>,
    ) -> Self {
        Self {
            source,
            applicability,
            normalized_value,
            confidence,
            evidence_ids,
        }
    }

    /// Returns the originating rule or criterion.
    pub const fn source(&self) -> &ContributionSource {
        &self.source
    }

    /// Returns the contribution applicability.
    pub const fn applicability(&self) -> RubricApplicability {
        self.applicability
    }

    /// Returns the normalized sub-score, absent only when not applicable.
    pub const fn normalized_value(&self) -> Option<f64> {
        self.normalized_value
    }

    pub(crate) fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns cited evidence for this contribution.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }
}
