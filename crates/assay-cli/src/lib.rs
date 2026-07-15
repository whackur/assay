//! Thin command-line delivery boundary for Assay.
//!
//! Command behavior is added in later vertical-slice work; product rules stay
//! in reusable crates.

#![forbid(unsafe_code)]

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
