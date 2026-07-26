//! Deterministic project type and maturity classification.
//! Maps cited evidence-grounded observations onto a primary type, secondary types, tags, and maturity — no I/O, byte-identical for identical input.
//! Observations are cited signals, not conclusions; absent signal yields explicit `unknown` rather than a default. A type-only result stays `unknown` pending a maturity signal.

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

// Re-exported so the public surface stays stable for callers that construct outcomes outside `classify_project`.
pub use applicability::criteria_applicability;
