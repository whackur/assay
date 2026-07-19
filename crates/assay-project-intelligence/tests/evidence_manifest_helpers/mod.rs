//! Shared helpers for the `evidence_manifest` test group.
//!
//! Each test file under `tests/` that needs these helpers declares
//! `mod evidence_manifest_helpers;` and imports the modules below.
//!
//! Helpers are compiled into every test binary that pulls this module in, and
//! each binary uses only a subset of them, so unused items and re-exports are
//! expected.
#![allow(dead_code)]
#![allow(unused_imports)]

mod common;
#[cfg(unix)]
mod edge;
mod policy;

pub use common::{
    classifications, collect_snapshot, feature, related_ids, snapshot, source, source_with_digest,
    trusted_git,
};
#[cfg(unix)]
pub use edge::edge_snapshot;
pub use policy::NamedPolicy;
