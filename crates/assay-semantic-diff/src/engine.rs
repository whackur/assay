//! The replaceable engine trait and the native tree-sitter adapter.

use tree_sitter::{Parser, Tree};

use crate::types::{
    EngineMetadata, Language, NATIVE_RULE_VERSION, ParseError, ParseSide, RawLineChanges,
    SemanticDiffInput, SemanticDiffResult,
};
use crate::units::{extract_units, match_units};

/// Replaceable interface for structural comparison engines.
pub trait SemanticDiffEngine {
    /// Returns version metadata for provenance and cache keys.
    fn metadata(&self) -> EngineMetadata;

    /// Compares source bytes without executing them.
    fn analyze(&self, input: SemanticDiffInput<'_>) -> SemanticDiffResult;
}

/// Selected first adapter: native Rust bindings over pinned tree-sitter grammars.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeTreeSitterEngine;

impl NativeTreeSitterEngine {
    /// Creates the stateless engine.
    pub const fn new() -> Self {
        Self
    }
}

impl SemanticDiffEngine for NativeTreeSitterEngine {
    fn metadata(&self) -> EngineMetadata {
        EngineMetadata {
            engine_id: "native-tree-sitter-1",
            parser_version: "tree-sitter-0.26.11",
            rule_version: NATIVE_RULE_VERSION,
        }
    }

    fn analyze(&self, input: SemanticDiffInput<'_>) -> SemanticDiffResult {
        let raw_lines = RawLineChanges {
            before_lines: logical_line_count(input.before),
            after_lines: logical_line_count(input.after),
            content_changed: input.before != input.after,
        };
        let before = parse(input.language, input.before);
        let after = parse(input.language, input.after);
        let mut parse_errors = Vec::new();
        if before.root_node().has_error() {
            parse_errors.push(ParseError {
                side: ParseSide::Before,
            });
        }
        if after.root_node().has_error() {
            parse_errors.push(ParseError {
                side: ParseSide::After,
            });
        }
        if !parse_errors.is_empty() {
            return SemanticDiffResult {
                operations: Vec::new(),
                parse_errors,
                raw_lines,
            };
        }

        let before_units = extract_units(input.language, &before, input.before);
        let after_units = extract_units(input.language, &after, input.after);
        SemanticDiffResult {
            operations: match_units(&before_units, &after_units),
            parse_errors,
            raw_lines,
        }
    }
}

fn logical_line_count(source: &[u8]) -> usize {
    if source.is_empty() {
        0
    } else {
        source.iter().filter(|byte| **byte == b'\n').count() + usize::from(!source.ends_with(b"\n"))
    }
}

fn parse(language: Language, source: &[u8]) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("pinned grammar ABI must match the pinned tree-sitter runtime");
    parser
        .parse(source, None)
        .expect("tree-sitter parsing without cancellation must return a tree")
}
