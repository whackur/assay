use assay_domain::ContentHash;

/// One named stage of the analysis pipeline, in interview-defined order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Stage {
    SourceVerification,
    RevisionPinning,
    FileAndHistoryAnalysis,
    ProjectTypeDetermination,
    CiAndDependencyEvidence,
    SimilarProjectDiscovery,
    AiRubricEvaluation,
    ScoreCompilation,
    ResultPublication,
}

/// The named pipeline stages in canonical execution order.
pub const PIPELINE_STAGES: [Stage; 9] = [
    Stage::SourceVerification,
    Stage::RevisionPinning,
    Stage::FileAndHistoryAnalysis,
    Stage::ProjectTypeDetermination,
    Stage::CiAndDependencyEvidence,
    Stage::SimilarProjectDiscovery,
    Stage::AiRubricEvaluation,
    Stage::ScoreCompilation,
    Stage::ResultPublication,
];

impl Stage {
    /// Returns the stable machine field name used in the public contract.
    pub const fn code(self) -> &'static str {
        match self {
            Self::SourceVerification => "source_verification",
            Self::RevisionPinning => "revision_pinning",
            Self::FileAndHistoryAnalysis => "file_and_history_analysis",
            Self::ProjectTypeDetermination => "project_type_determination",
            Self::CiAndDependencyEvidence => "ci_and_dependency_evidence",
            Self::SimilarProjectDiscovery => "similar_project_discovery",
            Self::AiRubricEvaluation => "ai_rubric_evaluation",
            Self::ScoreCompilation => "score_compilation",
            Self::ResultPublication => "result_publication",
        }
    }
}

/// The four-state lifecycle position of a stage or of the whole run.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StageStatus {
    /// Not yet settled: never attempted, or failed with retry budget remaining.
    Pending,
    /// Settled with a complete, reusable result snapshot.
    Complete,
    /// Settled with a usable result that has explicit gaps.
    Partial,
    /// Settled without a usable result after the retry budget was exhausted.
    Unavailable,
}

impl StageStatus {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }

    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Partial | Self::Unavailable)
    }

    pub(crate) const fn is_failed(self) -> bool {
        matches!(self, Self::Partial | Self::Unavailable)
    }
}

/// The outcome a worker reports for one bounded attempt at a stage.
#[derive(Clone, Debug, PartialEq)]
pub enum StageAttempt {
    /// The stage produced a complete, reusable result snapshot.
    Completed(ContentHash),
    /// The stage produced a usable result with explicit gaps and a reason.
    PartiallyCompleted {
        snapshot: ContentHash,
        reason: String,
    },
    /// The stage failed to produce a usable result, with a redacted reason.
    Failed { reason: String },
}

/// How the state machine settled a recorded attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptDisposition {
    /// The stage reached a terminal state (`complete` or `partial`).
    Settled,
    /// The stage failed but automatic retry budget remains; it stays `pending`.
    RetryScheduled,
    /// The stage failed and the automatic retry budget is now exhausted.
    Exhausted,
}
