//! Versioned, path-based file policy classification for Assay.
//!
//! The built-in policy measures reviewable path and resolved Git attribute
//! evidence. It does not inspect source contents, execute repository code, or
//! measure correctness, importance, human effort, productivity, or semantic
//! impact. A category describes the apparent role of a file; it is not a
//! quality judgment. In particular, [`ClassificationCategory::Unknown`] and
//! unavailable attribute facts must not be interpreted as zero value or
//! silently converted to production code.
//!
//! Repository-specific and organization-specific policy belongs behind the
//! [`ClassificationPolicy`] boundary. It is not embedded in the built-in Rust
//! rules.
//!
//! Module layout: types and behavior are split by responsibility (error,
//! path, attributes, input, categories, identifiers, confidence, evidence,
//! decision, policy boundary, built-in policy, path matchers, path dispatcher).
//! Public items are re-exported here so downstream crates and the public schema
//! contract remain stable.

#![forbid(unsafe_code)]

mod attributes;
mod built_in;
mod categories;
mod confidence;
mod decision;
mod error;
mod evidence;
mod identifiers;
mod input;
mod matchers;
mod path;
mod policy;
mod rules;

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// Stable version of the complete built-in file classification policy.
pub const BUILT_IN_RULE_SET_VERSION: &str = "file-classifier-1";

pub use attributes::{AttributeAvailability, LinguistAttributeFacts};
pub use built_in::BuiltInPolicy;
pub use categories::{ClassificationCategory, ClassificationTag};
pub use confidence::Confidence;
pub use decision::{ClassificationDecision, FileClassification};
pub use error::ClassificationError;
pub use evidence::{ClassificationEvidence, ClassificationEvidenceKind};
pub use identifiers::{PolicyVersion, RuleId};
pub use input::FileClassificationInput;
pub use path::PortablePath;
pub use policy::{ClassificationPolicy, classify_with_policy};
