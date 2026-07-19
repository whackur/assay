use assay_domain::EvidenceId;

use crate::EvaluationError;

use super::kind::EvidenceKind;
use super::text::{TextPolicy, validate_untrusted_text};

/// One citable, bounded statement derived from deterministic evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct EvidenceDescriptor {
    id: EvidenceId,
    kind: EvidenceKind,
    statement: String,
}

impl EvidenceDescriptor {
    /// Creates a provider-safe descriptor without raw source, diffs, secrets,
    /// host paths, prompt instructions, or person-level evaluation language.
    pub fn new(
        id: EvidenceId,
        kind: EvidenceKind,
        statement: &str,
    ) -> Result<Self, EvaluationError> {
        validate_untrusted_text(statement, TextPolicy::Evidence)?;
        Ok(Self {
            id,
            kind,
            statement: statement.to_owned(),
        })
    }

    /// Returns the stable evidence citation identifier.
    pub const fn id(&self) -> &EvidenceId {
        &self.id
    }

    /// Returns the bounded fact category.
    pub const fn kind(&self) -> EvidenceKind {
        self.kind
    }

    /// Returns the reviewed bounded statement, not source or raw diff text.
    pub fn statement(&self) -> &str {
        &self.statement
    }
}

impl std::fmt::Debug for EvidenceDescriptor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EvidenceDescriptor")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("statement", &"<bounded-evidence>")
            .finish()
    }
}
