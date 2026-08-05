//! Deterministic one-depth functional-cohort discovery and comparison.
//! Extracts a comparison profile, asks the candidate-search port for public GitHub candidates once, and compares each against the seed. A discovered candidate carries no profile, so it can never seed another pass.
//! Similarity uses declared facet tokens with deterministic integer arithmetic. Each mode has a closed canonical facet set enumerated on every comparison — a missing facet is explicit `unavailable`, never zero. A detailed candidate always carries at least one cited selection reason. Similarity is never a quality signal and never implies misconduct; star counts are ordering tie-breaks only. Curated lists are compared as artifacts, not by analyzing linked projects.

mod candidate;
mod cohort;
mod mapping;
mod policy;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use candidate::Candidate;
pub use cohort::{CohortComparison, discover_cohort};
pub use policy::ComparisonPolicy;
pub use types::{
    CandidateDescriptor, CandidateSearch, CandidateSearchError, CandidateSearchOutcome, CohortMode,
    CohortQuery, ComparisonError, ComparisonErrorKind, ComparisonProfile, SearchDepth, SeedProject,
};
