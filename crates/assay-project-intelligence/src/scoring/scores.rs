use assay_domain::{EvidenceId, EvidenceStatus};
use serde_json::{Value, json};

use crate::scoring::contribution::ScoreContribution;
use crate::scoring::dimensions::ScoreDimension;
use crate::scoring::mapping::{evidence_values, score_value, status_code};
use crate::scoring::statements::CitedStatement;

/// One compiled dimension score with its contribution breakdown.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionScore {
    pub(crate) dimension: ScoreDimension,
    pub(crate) status: EvidenceStatus,
    pub(crate) value: Option<f64>,
    pub(crate) confidence: f64,
    pub(crate) version: String,
    pub(crate) evidence_ids: Vec<EvidenceId>,
    pub(crate) contributions: Vec<ScoreContribution>,
}

impl DimensionScore {
    /// Returns the scored dimension.
    pub const fn dimension(&self) -> ScoreDimension {
        self.dimension
    }

    /// Returns availability; unavailable and insufficient are never zero scores.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` value, absent when not scoreable.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// Returns score confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns the explainable rule and criterion contributions.
    pub fn contributions(&self) -> &[ScoreContribution] {
        &self.contributions
    }

    pub(crate) fn to_value(&self) -> Value {
        score_value(
            self.status,
            self.value,
            self.confidence,
            &self.version,
            &self.evidence_ids,
        )
    }
}

/// The overall Assay Score, compiled from available dimensions only.
#[derive(Clone, Debug, PartialEq)]
pub struct AssayScore {
    pub(crate) status: EvidenceStatus,
    pub(crate) value: Option<f64>,
    pub(crate) confidence: f64,
    pub(crate) provisional: bool,
    pub(crate) version: String,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl AssayScore {
    /// Returns availability; a missing essential dimension keeps it unscored.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` value, absent when sufficiency is not met.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    /// Returns whether the value is a low-confidence provisional normalization.
    pub const fn provisional(&self) -> bool {
        self.provisional
    }

    /// Returns overall confidence in the closed unit interval.
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    pub(crate) fn to_value(&self) -> Value {
        score_value(
            self.status,
            self.value,
            self.confidence,
            &self.version,
            &self.evidence_ids,
        )
    }
}

/// The separate Potential indicator, never included in the Assay Score.
#[derive(Clone, Debug, PartialEq)]
pub struct PotentialScore {
    pub(crate) status: EvidenceStatus,
    pub(crate) value: Option<f64>,
    pub(crate) confidence: f64,
    pub(crate) version: String,
    pub(crate) evidence_ids: Vec<EvidenceId>,
    pub(crate) forecast_horizon: String,
    pub(crate) assumptions: Vec<CitedStatement>,
    pub(crate) major_counter_signals: Vec<CitedStatement>,
    pub(crate) contributions: Vec<ScoreContribution>,
}

impl PotentialScore {
    /// Returns Potential availability.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the `0..=100` Potential value, absent when not forecastable.
    pub const fn value(&self) -> Option<f64> {
        self.value
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "status": status_code(self.status),
            "value": self.value,
            "confidence": self.confidence,
            "version": self.version,
            "evidence_ids": evidence_values(&self.evidence_ids),
            "forecast_horizon": self.forecast_horizon,
            "assumptions": self.assumptions.iter().map(CitedStatement::to_value).collect::<Vec<_>>(),
            "major_counter_signals": self.major_counter_signals.iter().map(CitedStatement::to_value).collect::<Vec<_>>(),
        })
    }
}
