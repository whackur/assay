//! Structural change analysis behind a replaceable engine boundary.
//!
//! This crate compares caller-supplied source bytes. It never installs,
//! imports, builds, tests, or executes repository code. Structural operations
//! describe syntax-tree differences and must not be interpreted as human
//! effort, importance, correctness, or quality.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::{Language as TreeSitterLanguage, Node, Parser, Tree};

/// Version of Assay's syntax-unit extraction and matching rules.
pub const NATIVE_RULE_VERSION: &str = "semantic-unit-matcher-1";

/// Languages in the first semantic-diff boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Language {
    /// JavaScript source parsed with the JavaScript grammar.
    JavaScript,
    /// TypeScript source parsed with the TypeScript grammar.
    TypeScript,
    /// Python source parsed with the Python grammar.
    Python,
}

impl Language {
    fn grammar(self) -> TreeSitterLanguage {
        match self {
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }

    fn unit_kind(self) -> &'static str {
        match self {
            Self::JavaScript | Self::TypeScript => "function_declaration",
            Self::Python => "function_definition",
        }
    }
}

/// Borrowed input for a single-file comparison.
pub struct SemanticDiffInput<'source> {
    language: Language,
    before: &'source [u8],
    after: &'source [u8],
}

impl<'source> SemanticDiffInput<'source> {
    /// Creates an input without reading paths or executing source files.
    pub const fn new(language: Language, before: &'source [u8], after: &'source [u8]) -> Self {
        Self {
            language,
            before,
            after,
        }
    }
}

/// Stable structural operation categories.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChangeKind {
    /// A semantic unit exists only in the new source.
    Added,
    /// A semantic unit exists only in the old source.
    Removed,
    /// A same-named semantic unit has a different structural body.
    Modified,
    /// An otherwise unchanged unit changed top-level order.
    Moved,
    /// An otherwise unchanged unit changed its declared name.
    Renamed,
}

/// One bounded semantic-unit observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperation {
    kind: ChangeKind,
    before_name: Option<String>,
    after_name: Option<String>,
}

impl SemanticOperation {
    /// Returns the structural category.
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// Returns the old symbol name when one exists.
    pub fn before_name(&self) -> Option<&str> {
        self.before_name.as_deref()
    }

    /// Returns the new symbol name when one exists.
    pub fn after_name(&self) -> Option<&str> {
        self.after_name.as_deref()
    }
}

/// Which side of a comparison contained syntax errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseSide {
    /// The old source.
    Before,
    /// The new source.
    After,
}

/// A parse error summary that contains no source text or machine path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    side: ParseSide,
}

impl ParseError {
    /// Returns the affected input side.
    pub const fn side(self) -> ParseSide {
        self.side
    }
}

/// Raw line facts kept separate from semantic operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawLineChanges {
    before_lines: usize,
    after_lines: usize,
    content_changed: bool,
}

impl RawLineChanges {
    /// Returns the number of logical old-source lines.
    pub const fn before_lines(self) -> usize {
        self.before_lines
    }

    /// Returns the number of logical new-source lines.
    pub const fn after_lines(self) -> usize {
        self.after_lines
    }

    /// Returns whether the byte inputs differ.
    pub const fn content_changed(self) -> bool {
        self.content_changed
    }
}

/// Result from one engine invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDiffResult {
    operations: Vec<SemanticOperation>,
    parse_errors: Vec<ParseError>,
    raw_lines: RawLineChanges,
}

impl SemanticDiffResult {
    /// Returns structural operations in deterministic order.
    pub fn operations(&self) -> &[SemanticOperation] {
        &self.operations
    }

    /// Returns only the operation categories for compact contract assertions.
    pub fn kinds(&self) -> Vec<ChangeKind> {
        self.operations
            .iter()
            .map(SemanticOperation::kind)
            .collect()
    }

    /// Returns explicit parse failures. Callers must fall back to text facts.
    pub fn parse_errors(&self) -> &[ParseError] {
        &self.parse_errors
    }

    /// Returns raw byte/line facts independently from structural operations.
    pub const fn raw_lines(&self) -> RawLineChanges {
        self.raw_lines
    }
}

/// Version metadata that must accompany persisted engine observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineMetadata {
    engine_id: &'static str,
    parser_version: &'static str,
    rule_version: &'static str,
}

impl EngineMetadata {
    /// Returns the stable adapter identifier.
    pub const fn engine_id(&self) -> &'static str {
        self.engine_id
    }

    /// Returns the pinned tree-sitter Rust runtime version.
    pub const fn parser_version(&self) -> &'static str {
        self.parser_version
    }

    /// Returns the Assay extraction/matching rule version.
    pub const fn rule_version(&self) -> &'static str {
        self.rule_version
    }
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct Unit {
    name: String,
    structure: String,
    position: usize,
}

fn extract_units(language: Language, tree: &Tree, source: &[u8]) -> Vec<Unit> {
    let mut units = Vec::new();
    collect_units(language, tree.root_node(), source, &mut units);
    units.sort_by_key(|unit| unit.position);
    units
}

fn collect_units(language: Language, node: Node<'_>, source: &[u8], units: &mut Vec<Unit>) {
    if node.kind() == language.unit_kind() {
        if let Some(name_node) = node.child_by_field_name("name") {
            units.push(Unit {
                name: String::from_utf8_lossy(&source[name_node.byte_range()]).into_owned(),
                structure: canonical_structure(node, Some(name_node.id()), source),
                position: node.start_byte(),
            });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_units(language, child, source, units);
    }
}

fn canonical_structure(node: Node<'_>, omitted_node_id: Option<usize>, source: &[u8]) -> String {
    if omitted_node_id == Some(node.id()) {
        return "<declared-name>".to_owned();
    }

    let mut canonical = String::new();
    canonical.push('(');
    canonical.push_str(node.kind());
    let child_count = node.child_count();
    if child_count == 0 {
        if node.is_named() || is_semantic_anonymous_token(node, source) {
            canonical.push(':');
            canonical.push_str(&String::from_utf8_lossy(&source[node.byte_range()]));
        }
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() || is_semantic_anonymous_token(child, source) {
                canonical.push_str(&canonical_structure(child, omitted_node_id, source));
            }
        }
    }
    canonical.push(')');
    canonical
}

fn is_semantic_anonymous_token(node: Node<'_>, source: &[u8]) -> bool {
    let token = &source[node.byte_range()];
    !matches!(
        token,
        b"{" | b"}" | b"(" | b")" | b"[" | b"]" | b"," | b";" | b":"
    )
}

fn match_units(before: &[Unit], after: &[Unit]) -> Vec<SemanticOperation> {
    let mut operations = Vec::new();
    let before_by_name = unique_names(before);
    let after_by_name = unique_names(after);
    let mut matched_before = BTreeSet::new();
    let mut matched_after = BTreeSet::new();
    let mut unchanged_pairs = Vec::new();

    for (name, &before_index) in &before_by_name {
        let Some(&after_index) = after_by_name.get(name) else {
            continue;
        };
        matched_before.insert(before_index);
        matched_after.insert(after_index);
        if before[before_index].structure == after[after_index].structure {
            unchanged_pairs.push((before_index, after_index));
        } else {
            operations.push(operation(ChangeKind::Modified, Some(name), Some(name)));
        }
    }

    let before_unmatched = unmatched_indices(before.len(), &matched_before);
    let after_unmatched = unmatched_indices(after.len(), &matched_after);
    let mut renamed_before = BTreeSet::new();
    let mut renamed_after = BTreeSet::new();
    for &before_index in &before_unmatched {
        let candidates = after_unmatched
            .iter()
            .copied()
            .filter(|after_index| {
                !renamed_after.contains(after_index)
                    && before[before_index].structure == after[*after_index].structure
            })
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            let after_index = candidates[0];
            renamed_before.insert(before_index);
            renamed_after.insert(after_index);
            operations.push(operation(
                ChangeKind::Renamed,
                Some(&before[before_index].name),
                Some(&after[after_index].name),
            ));
        }
    }

    unchanged_pairs.sort_unstable();
    let after_sequence = unchanged_pairs
        .iter()
        .map(|(_, after_index)| *after_index)
        .collect::<Vec<_>>();
    let stable_pair_indices = longest_increasing_subsequence_indices(&after_sequence);
    for (pair_index, (before_index, after_index)) in unchanged_pairs.iter().enumerate() {
        if !stable_pair_indices.contains(&pair_index) {
            operations.push(operation(
                ChangeKind::Moved,
                Some(&before[*before_index].name),
                Some(&after[*after_index].name),
            ));
        }
    }

    for before_index in before_unmatched {
        if !renamed_before.contains(&before_index) {
            operations.push(operation(
                ChangeKind::Removed,
                Some(&before[before_index].name),
                None,
            ));
        }
    }
    for after_index in after_unmatched {
        if !renamed_after.contains(&after_index) {
            operations.push(operation(
                ChangeKind::Added,
                None,
                Some(&after[after_index].name),
            ));
        }
    }

    operations.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.before_name.cmp(&right.before_name))
            .then_with(|| left.after_name.cmp(&right.after_name))
    });
    operations
}

fn unique_names(units: &[Unit]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for unit in units {
        *counts.entry(unit.name.as_str()).or_insert(0_usize) += 1;
    }
    units
        .iter()
        .enumerate()
        .filter(|(_, unit)| counts.get(unit.name.as_str()) == Some(&1))
        .map(|(index, unit)| (unit.name.as_str(), index))
        .collect()
}

fn unmatched_indices(length: usize, matched: &BTreeSet<usize>) -> Vec<usize> {
    (0..length)
        .filter(|index| !matched.contains(index))
        .collect()
}

fn operation(
    kind: ChangeKind,
    before_name: Option<&str>,
    after_name: Option<&str>,
) -> SemanticOperation {
    SemanticOperation {
        kind,
        before_name: before_name.map(str::to_owned),
        after_name: after_name.map(str::to_owned),
    }
}

fn longest_increasing_subsequence_indices(values: &[usize]) -> BTreeSet<usize> {
    let mut lengths = vec![1_usize; values.len()];
    let mut previous = vec![None; values.len()];
    for current in 0..values.len() {
        for prior in 0..current {
            if values[prior] < values[current] && lengths[prior] + 1 > lengths[current] {
                lengths[current] = lengths[prior] + 1;
                previous[current] = Some(prior);
            }
        }
    }
    let Some(mut cursor) = (0..values.len()).max_by_key(|index| lengths[*index]) else {
        return BTreeSet::new();
    };
    let mut stable = BTreeSet::new();
    loop {
        stable.insert(cursor);
        let Some(prior) = previous[cursor] else {
            break;
        };
        cursor = prior;
    }
    stable
}
