use assay_domain::EvidenceId;
use serde_json::{Value, json};

use crate::scoring::error::{ScoreCompileError, ScoreCompileErrorKind};
use crate::scoring::mapping::evidence_values;
use crate::scoring::validation::{is_statement, sorted_unique};

/// A cited factual assumption or counter-signal statement.
#[derive(Clone, Debug, PartialEq)]
pub struct CitedStatement {
    pub(crate) text: String,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl CitedStatement {
    /// Validates a bounded statement that cites at least one evidence identifier.
    pub fn new(text: &str, evidence_ids: Vec<EvidenceId>) -> Result<Self, ScoreCompileError> {
        if !is_statement(text) || evidence_ids.is_empty() {
            return Err(ScoreCompileError::new(
                ScoreCompileErrorKind::InvalidStatement,
            ));
        }
        Ok(Self {
            text: text.to_owned(),
            evidence_ids: sorted_unique(evidence_ids),
        })
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({ "text": self.text, "evidence_ids": evidence_values(&self.evidence_ids) })
    }
}

/// Separately supplied cited context for the Potential forecast.
///
/// The compiler validates citations and passes the narrative through; it does
/// not invent Potential prose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PotentialContext {
    pub(crate) assumptions: Vec<CitedStatement>,
    pub(crate) major_counter_signals: Vec<CitedStatement>,
}

impl PotentialContext {
    /// Creates cited Potential context.
    pub fn new(
        assumptions: Vec<CitedStatement>,
        major_counter_signals: Vec<CitedStatement>,
    ) -> Self {
        Self {
            assumptions,
            major_counter_signals,
        }
    }
}
