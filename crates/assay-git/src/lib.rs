//! Git repository history extraction for Assay.
//!
//! Collection implementations depend on domain contracts and must never
//! execute code from an analyzed repository.

#![forbid(unsafe_code)]

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
