//! Structural change analysis behind a replaceable engine boundary.
//!
//! This crate compares caller-supplied source bytes. It never installs,
//! imports, builds, tests, or executes repository code. Structural operations
//! describe syntax-tree differences and must not be interpreted as human
//! effort, importance, correctness, or quality.

#![forbid(unsafe_code)]

mod engine;
mod types;
mod units;

pub use engine::{NativeTreeSitterEngine, SemanticDiffEngine};
pub use types::{
    ChangeKind, EngineMetadata, Language, NATIVE_RULE_VERSION, ParseError, ParseSide,
    RawLineChanges, SemanticDiffInput, SemanticDiffResult, SemanticOperation,
};
