//! Public domain types for the semantic-diff boundary.
//!
//! These types describe structural syntax-tree differences. They must never
//! be interpreted as human effort, importance, correctness, or quality.

use tree_sitter::Language as TreeSitterLanguage;

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
    pub(crate) fn grammar(self) -> TreeSitterLanguage {
        match self {
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }

    pub(crate) fn unit_kind(self) -> &'static str {
        match self {
            Self::JavaScript | Self::TypeScript => "function_declaration",
            Self::Python => "function_definition",
        }
    }
}

/// Borrowed input for a single-file comparison.
pub struct SemanticDiffInput<'source> {
    pub(crate) language: Language,
    pub(crate) before: &'source [u8],
    pub(crate) after: &'source [u8],
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
    pub(crate) kind: ChangeKind,
    pub(crate) before_name: Option<String>,
    pub(crate) after_name: Option<String>,
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
    pub(crate) side: ParseSide,
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
    pub(crate) before_lines: usize,
    pub(crate) after_lines: usize,
    pub(crate) content_changed: bool,
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
    pub(crate) operations: Vec<SemanticOperation>,
    pub(crate) parse_errors: Vec<ParseError>,
    pub(crate) raw_lines: RawLineChanges,
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
    pub(crate) engine_id: &'static str,
    pub(crate) parser_version: &'static str,
    pub(crate) rule_version: &'static str,
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
