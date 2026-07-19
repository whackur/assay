//! Shared helpers for the `score_compiler` test group.
//!
//! Each test file under `tests/` that needs these helpers declares
//! `mod score_compiler_helpers;` and imports the modules below.
//!
//! Helpers are compiled into every test binary that pulls this module in, and
//! each binary uses only a subset of them, so unused items and re-exports are
//! expected.
#![allow(dead_code)]
#![allow(unused_imports)]

mod fixtures;
mod schema;

pub use fixtures::{
    contribution, deterministic_evaluator, essential_contributions, evidence,
    golden_classification, golden_input, golden_potential_context, project_source, revision,
    snapshot_evidence,
};
pub use schema::{assert_schema_valid, evaluation_schema, repository_root};
