//! Deterministic one-depth functional-cohort discovery and comparison.
//!
//! Assay extracts a comparison profile from the analyzed project, asks a narrow
//! candidate-search port for public GitHub candidates exactly once, and compares
//! each candidate against the seed. Discovery stops at one search depth: a
//! discovered candidate carries no profile and cannot construct a
//! [`CohortQuery`], so it can never seed another discovery pass. The real GitHub
//! search wiring lives behind [`CandidateSearch`] and is deferred; this module
//! is exercised with deterministic fakes.
//!
//! Similarity is computed only from declared facet tokens with deterministic
//! integer arithmetic; identical input yields byte-identical output. Each mode
//! has a closed canonical facet set that every comparison enumerates — a facet
//! without data on either side is explicitly unavailable, never a zero — and a
//! detailed candidate always carries at least one cited selection reason.
//! Similarity is never a quality signal and never implies misconduct. Popularity such as
//! star counts is retained as context and used only as an ordering tie-break;
//! it never raises a similarity value. An awesome list is compared as a curated
//! artifact against other curated lists, never by analyzing its linked projects.
//! Unavailable and insufficient comparisons remain explicit states, never zero.

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
