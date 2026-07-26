//! Deterministic project score compiler.
//! Combines deterministic per-dimension rule contributions with validated rubric judgments into dimensioned, confidence-aware scores implementing `schemas/project-evaluation/v1.json`. No I/O; byte-identical for identical input.
//! A provider influences a score only through a bounded [`RubricJudgment`] rating — never emits or overrides a dimension or the overall Assay Score. `not_applicable` and unavailable checks never become zero. Popularity signals have no input. Potential is compiled separately, never included in the Assay Score. Weights and the sufficiency rule are versioned policy folded into the published rule-set hash.

mod classification;
mod compile_stages;
mod compiler;
mod contribution;
mod dimensions;
mod enums;
mod error;
mod evaluation;
mod evaluator;
mod mapping;
mod policy;
mod scores;
mod statements;
mod validation;

pub use classification::ProjectClassification;
pub use compiler::ScoreCompilerInput;
pub use contribution::{ContributionSource, DeterministicContribution, ScoreContribution};
pub use dimensions::{ASSAY_SCORE_DIMENSIONS, ScoreDimension};
pub use enums::{EvaluatorProvider, ProjectMaturity, ProjectType, Visibility};
pub use error::{ScoreCompileError, ScoreCompileErrorKind};
pub use evaluation::CompiledEvaluation;
pub use evaluator::EvaluatorDescriptor;
pub use policy::CompilerPolicy;
pub use scores::{AssayScore, DimensionScore, PotentialScore};
pub use statements::{CitedStatement, PotentialContext};
