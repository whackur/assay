//! Shared helpers for the `machine_contract` test group.
//!
//! Each test file under `tests/` that needs these helpers declares
//! `mod machine_contract_helpers;` and imports the modules below.
//!
//! Helpers are compiled into every test binary that pulls this module in, and
//! each binary uses only a subset of them, so unused items and re-exports are
//! expected.
#![allow(dead_code)]
#![allow(unused_imports)]

mod bundles;
mod features;
mod ids;

pub use bundles::{
    bundle_with_present_feature, coherent_bundle, real_producer_bundle, tracked_file_record,
};
pub use features::{
    feature, feature_mut, feature_related_ids, refresh_project_artifact, set_feature,
};
pub use ids::repository_feature_id;
