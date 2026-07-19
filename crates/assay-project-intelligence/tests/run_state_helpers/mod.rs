//! Shared helpers for the `run_state` test group.
//!
//! Each test file under `tests/` that needs these helpers declares
//! `mod run_state_helpers;` and imports the modules below.
//!
//! Helpers are compiled into every test binary that pulls this module in, and
//! each binary uses only a subset of them, so unused items and re-exports are
//! expected.
#![allow(dead_code)]
#![allow(unused_imports)]

mod fixtures;
mod schema;

pub use fixtures::{completed, exhaust, new_run, representative_run, run_id, snapshot};
pub use schema::{repository_root, run_state_schema};
