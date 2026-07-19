use std::{error::Error, fmt};

/// Stable, redacted score-compilation failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreCompileErrorKind {
    InvalidContribution,
    InvalidClassification,
    InvalidStatement,
    InvalidEvaluator,
    UnknownCriterionDimension,
    RubricVersionMismatch,
}

/// A redacted compilation failure that never echoes source or path material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreCompileError {
    kind: ScoreCompileErrorKind,
}

impl ScoreCompileError {
    pub(crate) const fn new(kind: ScoreCompileErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> ScoreCompileErrorKind {
        self.kind
    }
}

impl fmt::Display for ScoreCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "score compilation failed ({:?})", self.kind)
    }
}

impl Error for ScoreCompileError {}
