//! Core domain contracts for Assay.
//!
//! This crate remains independent of database, HTTP, GitHub, CLI, and UI
//! concerns.

#![forbid(unsafe_code)]

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
