//! Deterministic project type and maturity classification.
//!
//! The classifier maps cited, evidence-grounded observations onto a primary
//! project type, optional secondary types, descriptive tags, and a maturity,
//! each with confidence and explicit `unknown` behavior. It performs no
//! filesystem, process, network, clock, or model-provider I/O; identical input
//! yields byte-identical output.
//!
//! An observation is a concrete cited signal, never a conclusion, so the
//! decision logic stays honest and reviewable. When no rule fires, or when the
//! type or maturity signal is absent, the result is an explicit `unknown`
//! classification rather than an invented default. A type label selects
//! applicable rubric criteria; it never by itself defines the comparison cohort.
//!
//! The emitted [`ProjectClassification`] is the exact value the deterministic
//! score compiler consumes, so classification and scoring stay aligned. The
//! evaluation schema requires a resolved maturity alongside a resolved type for
//! a usable classification, so a type-only result is intentionally represented
//! as `unknown` pending a maturity signal.

mod applicability;
mod classify;
mod error;
mod observations;
mod outcome;
mod policy;
mod signals;

#[cfg(test)]
mod tests;

pub use classify::classify_project;
pub use error::{ClassificationError, ClassificationErrorKind};
pub use observations::{MaturityObservation, TypeObservation};
pub use outcome::ClassificationOutcome;
pub use policy::ClassificationPolicy;
pub use signals::{MaturitySignal, TypeSignal};

// Re-export the applicability resolver so the public surface stays stable for
// callers that construct outcomes outside `classify_project`.
pub use applicability::criteria_applicability;
