//! Deterministic project score compiler.
//!
//! The compiler combines deterministic per-dimension rule contributions with
//! validated qualitative rubric judgments into dimensioned, confidence-aware
//! scores that implement `schemas/project-evaluation/v1.json`. It performs no
//! filesystem, process, network, clock, or model-provider I/O; identical input
//! yields byte-identical output.
//!
//! A provider can influence a score only through a bounded [`RubricJudgment`]
//! rating; it can never emit or override a dimension or the overall Assay Score.
//! `not_applicable` and unavailable checks never become a zero score. Popularity
//! signals such as stars, forks, and downloads have no input to the compiler and
//! therefore cannot raise a score. Potential is compiled separately and is never
//! included in the Assay Score. Weights and the sufficiency rule are versioned
//! policy data folded into the published rule-set hash, not scattered constants.

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
