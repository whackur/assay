use serde::{Deserialize, Serialize};

/// Availability of one evidence source, independent from analysis status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Complete,
    /// Some usable evidence collected, with explicit gaps.
    Partial,
    /// Evidence could not be obtained from the requested source.
    Unavailable,
    /// Analyzer does not support this evidence source or content.
    Unsupported,
    /// Evidence exists but is not sufficient for the requested interpretation.
    Insufficient,
    /// Evidence collection or maturation is not final yet.
    Pending,
}

/// Overall derived-analysis state, kept separate from raw evidence availability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Complete,
    /// Usable result completed with explicit gaps.
    Partial,
    /// Analysis could not produce a usable result because required input was unavailable.
    Unavailable,
    /// Requested analysis is not supported.
    Unsupported,
    /// Collected inputs are insufficient for the requested analysis.
    Insufficient,
    /// Analysis or maturation is not final yet.
    Pending,
}

/// Stable category for evidence provenance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceKind {
    Repository,
    RepositoryContent,
    RepositoryHistory,
    PlatformRecord,
    ReportedCi,
    ReleaseArtifact,
    Documentation,
}
