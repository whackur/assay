//! Versioned, evidence-bounded qualitative evaluation for Assay projects.
//!
//! Provider output and provider prose are untrusted until [`Evaluator`]
//! validates their schema-shaped fields, rubric membership, bounded ratings,
//! and citations against the exact [`EvidenceBundle`]. The validated scoring
//! view intentionally excludes provider rationale. This crate performs no
//! network, filesystem, process, credential, or score-compilation work.

#![forbid(unsafe_code)]

mod bundle;
mod error;
mod evaluator;
mod rubric;

pub use bundle::{
    EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope, ExternalTransmission,
};
pub use error::{EvaluationError, EvaluationErrorKind, ProviderError};
pub use evaluator::{
    Applicability, DeterministicFakeProvider, EvaluationProvider, EvaluationStatus, Evaluator,
    ProviderExecutionBoundary, ProviderRequest, ScoringJudgment, ValidatedJudgment,
    ValidatedJudgmentSet,
};
pub use rubric::{QualitativeCriterion, QualitativeRubric};

/// Stable public schema version produced by validated evaluator results.
pub const AI_JUDGMENT_SCHEMA_VERSION: &str = "1.0.0";

/// Stable evaluation version required by the initial project rubric.
pub const PROJECT_EVALUATION_VERSION: &str = "project-intelligence-1";

/// Stable prompt-envelope version shared by provider adapters.
pub const PROMPT_VERSION: &str = "project-evaluation-prompt-1";
