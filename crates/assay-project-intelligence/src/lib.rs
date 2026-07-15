//! Project-level evidence and evaluation contracts for Assay.
//!
//! This crate consumes domain, Git, and classification facts without mixing
//! project evaluation with person-level contribution metrics.

#![forbid(unsafe_code)]

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
