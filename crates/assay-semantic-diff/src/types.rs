//! Public domain types for the semantic-diff boundary.
//!
//! Structural syntax-tree differences only — never interpret as effort, importance, correctness, or quality.

use tree_sitter::Language as TreeSitterLanguage;

/// Version of Assay's syntax-unit extraction and matching rules.
pub const NATIVE_RULE_VERSION: &str = "semantic-unit-matcher-1";

/// Languages in the first semantic-diff boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Language {
    JavaScript,
    TypeScript,
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
    Added,
    Removed,
    /// Same-named semantic unit with a different structural body.
    Modified,
    /// Otherwise unchanged unit changed top-level order.
    Moved,
    /// Otherwise unchanged unit changed its declared name.
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
    pub const fn kind(&self) -> ChangeKind {
        self.kind
    }

    pub fn before_name(&self) -> Option<&str> {
        self.before_name.as_deref()
    }

    pub fn after_name(&self) -> Option<&str> {
        self.after_name.as_deref()
    }
}

/// Which side of a comparison contained syntax errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseSide {
    Before,
    After,
}

/// Parse error summary containing no source text or machine path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub(crate) side: ParseSide,
}

impl ParseError {
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
    pub const fn before_lines(self) -> usize {
        self.before_lines
    }

    pub const fn after_lines(self) -> usize {
        self.after_lines
    }

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
    /// Structural operations in deterministic order.
    pub fn operations(&self) -> &[SemanticOperation] {
        &self.operations
    }

    /// Only the operation categories, for compact contract assertions.
    pub fn kinds(&self) -> Vec<ChangeKind> {
        self.operations
            .iter()
            .map(SemanticOperation::kind)
            .collect()
    }

    /// Explicit parse failures. Callers must fall back to text facts.
    pub fn parse_errors(&self) -> &[ParseError] {
        &self.parse_errors
    }

    /// Raw byte/line facts independent from structural operations.
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
    pub const fn engine_id(&self) -> &'static str {
        self.engine_id
    }

    pub const fn parser_version(&self) -> &'static str {
        self.parser_version
    }

    pub const fn rule_version(&self) -> &'static str {
        self.rule_version
    }
}
