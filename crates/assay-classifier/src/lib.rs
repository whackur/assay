//! Versioned, path-based file policy classification for Assay.
//!
//! The built-in policy measures reviewable path and resolved Git attribute
//! evidence only — never source contents, executed code, correctness,
//! importance, effort, productivity, or semantic impact. A category describes
//! the apparent role of a file, not a quality judgment. In particular,
//! [`ClassificationCategory::Unknown`] and unavailable attribute facts must not
//! be interpreted as zero value or silently converted to production code.
//!
//! Repository- and organization-specific policy belongs behind the
//! [`ClassificationPolicy`] boundary, not in the built-in Rust rules.

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
