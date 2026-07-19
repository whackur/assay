//! Core domain contracts for Assay.
//!
//! This crate remains independent of database, HTTP, GitHub, CLI, and UI
//! concerns.

#![forbid(unsafe_code)]

mod error;
mod evidence;
mod hashes;
mod identifiers;
mod judgment;
mod judgment_applicability;
mod judgment_criterion;
mod judgment_set;
mod machine_code;
mod manifest;
mod repository;
mod snapshot;
mod status;
mod warning;

#[cfg(test)]
mod judgment_tests;

pub use error::DomainValueError;
pub use evidence::EvidenceSource;
pub use hashes::ContentHash;
pub use identifiers::{AnalysisVersion, EvidenceId, RevisionId, RuleSetHash};
pub use judgment::RubricJudgment;
pub use judgment_applicability::RubricApplicability;
pub use judgment_criterion::RubricCriterionId;
pub use judgment_set::RubricJudgmentSet;
pub use manifest::AnalysisManifest;
pub use repository::RepositorySource;
pub use snapshot::SourceSnapshot;
pub use status::{AnalysisStatus, EvidenceSourceKind, EvidenceStatus};
pub use warning::{Limitation, Warning};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
