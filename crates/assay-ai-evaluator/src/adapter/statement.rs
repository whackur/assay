//! Bounded statement generation for the manifest-to-bundle adapter.
//!
//! Each statement is derived from the fact's kind and availability, never from
//! the underlying content. Statements are short, factual, and free of raw
//! source, diffs, host paths, credentials, or person-level language so they
//! pass the untrusted-text policy in [`EvidenceDescriptor::new`].

use assay_domain::EvidenceStatus;
use assay_project_intelligence::{
    ClassificationCategoryRecord, ClassificationEvidenceRecord, RawEvidenceFact, RawEvidenceKind,
};

use crate::{EvaluationError, EvaluationErrorKind};

/// Adapter failure categories. Stable, redacted, and never content-bearing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestAdapterError {
    /// The manifest carried no raw or classification facts.
    EmptyManifest,
    /// A bounded statement violated the untrusted-text policy.
    EvidenceText(EvaluationErrorKind),
}

impl ManifestAdapterError {
    pub(crate) fn from_evaluation_error(error: EvaluationError) -> Self {
        Self::EvidenceText(error.kind())
    }
}

impl From<EvaluationError> for ManifestAdapterError {
    fn from(error: EvaluationError) -> Self {
        Self::from_evaluation_error(error)
    }
}

impl std::fmt::Display for ManifestAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyManifest => formatter.write_str("empty_evidence_manifest"),
            Self::EvidenceText(kind) => formatter.write_str(kind.code()),
        }
    }
}

impl std::error::Error for ManifestAdapterError {}

pub(crate) fn raw_statement(fact: &RawEvidenceFact) -> Result<String, EvaluationError> {
    let base = match fact.kind() {
        RawEvidenceKind::RepositorySnapshot => "The analyzed source revision is immutable.",
        RawEvidenceKind::TrackedFile => tracked_file_statement(fact),
        RawEvidenceKind::HistoryScope => "The repository history scope was collected.",
        RawEvidenceKind::ParentDelta => "The first-parent delta was collected.",
    };
    Ok(base.to_owned())
}

fn tracked_file_statement(fact: &RawEvidenceFact) -> &'static str {
    match fact.status() {
        EvidenceStatus::Complete => "A tracked file was collected with its content digest.",
        EvidenceStatus::Partial => "A tracked file was collected with partial content.",
        _ => "A tracked file was observed but its content is unavailable.",
    }
}

pub(crate) fn classification_statement(
    fact: &ClassificationEvidenceRecord,
    category: ClassificationCategoryRecord,
) -> Result<String, EvaluationError> {
    let category_name = category_label(category);
    let statement = match fact.status() {
        EvidenceStatus::Complete => {
            format!("A tracked file was classified as {category_name}.")
        }
        EvidenceStatus::Partial => format!(
            "A tracked file was partially classified as {category_name}; attribute resolution is unavailable."
        ),
        _ => "A tracked file classification was attempted but is unavailable.".to_owned(),
    };
    Ok(statement)
}

fn category_label(category: ClassificationCategoryRecord) -> &'static str {
    match category {
        ClassificationCategoryRecord::ProductionCode => "production code",
        ClassificationCategoryRecord::Test => "test code",
        ClassificationCategoryRecord::Documentation => "documentation",
        ClassificationCategoryRecord::CiCd => "CI/CD configuration",
        ClassificationCategoryRecord::Infrastructure => "infrastructure",
        ClassificationCategoryRecord::SchemaMigration => "schema migration",
        ClassificationCategoryRecord::Dependency => "dependency manifest",
        ClassificationCategoryRecord::SecurityPolicy => "security policy",
        ClassificationCategoryRecord::Configuration => "configuration",
        ClassificationCategoryRecord::Generated => "generated content",
        ClassificationCategoryRecord::Vendored => "vendored content",
        ClassificationCategoryRecord::BuildOutput => "build output",
        ClassificationCategoryRecord::Coverage => "coverage report",
        ClassificationCategoryRecord::Unknown => "unknown category",
    }
}
