use assay_domain::{EvidenceId, EvidenceStatus};
use serde_json::{Value, json};

use crate::scoring::enums::{ProjectMaturity, ProjectType};
use crate::scoring::error::{ScoreCompileError, ScoreCompileErrorKind};
use crate::scoring::mapping::{evidence_values, status_code};
use crate::scoring::validation::{is_machine_code, sorted_unique};

/// A classification supplied to the compiler by an upstream classifier stage.
///
/// The compiler consumes resolved applicability; it does not itself classify.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectClassification {
    pub(crate) status: EvidenceStatus,
    pub(crate) primary_type: Option<ProjectType>,
    pub(crate) secondary_types: Vec<ProjectType>,
    pub(crate) tags: Vec<String>,
    pub(crate) maturity: Option<ProjectMaturity>,
    pub(crate) confidence: f64,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl ProjectClassification {
    /// Validates a classification whose type and maturity presence match status.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        status: EvidenceStatus,
        primary_type: Option<ProjectType>,
        secondary_types: Vec<ProjectType>,
        tags: Vec<String>,
        maturity: Option<ProjectMaturity>,
        confidence: f64,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ScoreCompileError> {
        let invalid = |()| ScoreCompileError::new(ScoreCompileErrorKind::InvalidClassification);
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(invalid(()));
        }
        if !tags.iter().all(|tag| is_machine_code(tag)) {
            return Err(invalid(()));
        }
        let usable = matches!(status, EvidenceStatus::Complete | EvidenceStatus::Partial);
        if usable != (primary_type.is_some() && maturity.is_some()) {
            return Err(invalid(()));
        }
        if usable && evidence_ids.is_empty() {
            return Err(invalid(()));
        }
        Ok(Self {
            status,
            primary_type,
            secondary_types,
            tags,
            maturity,
            confidence,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    /// Returns cited classification evidence in canonical order.
    pub fn evidence_ids(&self) -> &[EvidenceId] {
        &self.evidence_ids
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "status": status_code(self.status),
            "primary_type": self.primary_type.map(ProjectType::code),
            "secondary_types": self.secondary_types.iter().map(|value| value.code()).collect::<Vec<_>>(),
            "tags": self.tags,
            "maturity": self.maturity.map(ProjectMaturity::code),
            "confidence": self.confidence,
            "evidence_ids": evidence_values(&self.evidence_ids),
        })
    }
}
