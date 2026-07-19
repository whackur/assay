use assay_domain::EvidenceId;

use crate::classification::error::{ClassificationError, ClassificationErrorKind};
use crate::classification::signals::{MaturitySignal, TypeSignal};

fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}

/// One cited type observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeObservation {
    pub(crate) signal: TypeSignal,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl TypeObservation {
    /// Validates one type observation, requiring at least one citation.
    pub fn new(
        signal: TypeSignal,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ClassificationError> {
        if evidence_ids.is_empty() {
            return Err(ClassificationError {
                kind: ClassificationErrorKind::UncitedObservation,
            });
        }
        Ok(Self {
            signal,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }
}

/// One cited maturity observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaturityObservation {
    pub(crate) signal: MaturitySignal,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl MaturityObservation {
    /// Validates one maturity observation, requiring at least one citation.
    pub fn new(
        signal: MaturitySignal,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ClassificationError> {
        if evidence_ids.is_empty() {
            return Err(ClassificationError {
                kind: ClassificationErrorKind::UncitedObservation,
            });
        }
        Ok(Self {
            signal,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }
}
