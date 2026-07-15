//! File and change policy classification for Assay.
//!
//! Classification consumes domain facts and keeps generated, vendored, and
//! other file categories explicit.

#![forbid(unsafe_code)]

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
