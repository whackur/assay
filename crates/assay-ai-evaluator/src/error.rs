use std::{error::Error, fmt};

/// Stable failure category for bundle construction and provider validation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvaluationErrorKind {
    EmptyEvidenceBundle,
    DuplicateEvidence,
    EvidenceTextInvalid,
    PromptInjection,
    SensitiveContent,
    AbsolutePath,
    RawDiff,
    PersonDomainMixing,
    PrivacyMismatch,
    ProviderFailure,
    ProviderTimeout,
    ProviderRateLimited,
    ProviderUnauthorized,
    SecretUnavailable,
    OutputTooLarge,
    MalformedOutput,
    SchemaInvalid,
    EvaluationVersionMismatch,
    RubricVersionMismatch,
    EvidenceBundleMismatch,
    UnknownCriterion,
    DuplicateCriterion,
    MissingCriterion,
    InvalidRating,
    InvalidConfidence,
    MissingCitation,
    DuplicateCitation,
    UnknownEvidenceCitation,
}

impl EvaluationErrorKind {
    /// Returns the stable machine-readable status code for this category.
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyEvidenceBundle => "empty_evidence_bundle",
            Self::DuplicateEvidence => "duplicate_evidence",
            Self::EvidenceTextInvalid => "evidence_text_invalid",
            Self::PromptInjection => "prompt_injection",
            Self::SensitiveContent => "sensitive_content",
            Self::AbsolutePath => "absolute_path",
            Self::RawDiff => "raw_diff",
            Self::PersonDomainMixing => "person_domain_mixing",
            Self::PrivacyMismatch => "privacy_mismatch",
            Self::ProviderFailure => "provider_failure",
            Self::ProviderTimeout => "provider_timeout",
            Self::ProviderRateLimited => "provider_rate_limited",
            Self::ProviderUnauthorized => "provider_unauthorized",
            Self::SecretUnavailable => "secret_unavailable",
            Self::OutputTooLarge => "output_too_large",
            Self::MalformedOutput => "malformed_output",
            Self::SchemaInvalid => "schema_invalid",
            Self::EvaluationVersionMismatch => "evaluation_version_mismatch",
            Self::RubricVersionMismatch => "rubric_version_mismatch",
            Self::EvidenceBundleMismatch => "evidence_bundle_mismatch",
            Self::UnknownCriterion => "unknown_criterion",
            Self::DuplicateCriterion => "duplicate_criterion",
            Self::MissingCriterion => "missing_criterion",
            Self::InvalidRating => "invalid_rating",
            Self::InvalidConfidence => "invalid_confidence",
            Self::MissingCitation => "missing_citation",
            Self::DuplicateCitation => "duplicate_citation",
            Self::UnknownEvidenceCitation => "unknown_evidence_citation",
        }
    }
}

/// Redacted evaluator failure that never retains provider or repository text.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EvaluationError {
    kind: EvaluationErrorKind,
}

impl EvaluationError {
    pub(crate) const fn new(kind: EvaluationErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable machine-readable failure category.
    pub const fn kind(&self) -> EvaluationErrorKind {
        self.kind
    }
}

impl fmt::Debug for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvaluationError")
            .field("code", &self.kind.code())
            .finish()
    }
}

impl fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.kind.code())
    }
}

impl Error for EvaluationError {}

/// Redacted provider adapter failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderError;

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("provider_failure")
    }
}

impl Error for ProviderError {}
