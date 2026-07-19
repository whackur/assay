use serde::Serialize;

/// Bounded evidence category supplied to a qualitative evaluator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    RepositoryFact,
    DocumentationClaim,
    ImplementationFact,
    Test,
    ReportedCi,
    ReleaseFact,
    RepositoryConfiguration,
    ComparisonFact,
}

impl EvidenceKind {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::RepositoryFact => "repository_fact",
            Self::DocumentationClaim => "documentation_claim",
            Self::ImplementationFact => "implementation_fact",
            Self::Test => "test",
            Self::ReportedCi => "reported_ci",
            Self::ReleaseFact => "release_fact",
            Self::RepositoryConfiguration => "repository_configuration",
            Self::ComparisonFact => "comparison_fact",
        }
    }
}
