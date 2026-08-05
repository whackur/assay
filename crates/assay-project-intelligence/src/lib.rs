//! Deterministic project-level evidence assembly for Assay.
//! Combines immutable Git snapshot facts with versioned file classification facts. No I/O. The typed manifest is not a serialized public contract; CLI mapping to `schemas/` is a later boundary.
//! The manifest measures which facts were collected and how files were classified — not whether a project builds, works, is safe, original, or valuable. Never a project score, person-level observation, or contributor-intent signal. Missing/partial/unsupported evidence stays explicit, never numeric zero.

#![forbid(unsafe_code)]

mod ai_analysis;
mod classification;
mod comparison;
mod contract;
mod evidence;
mod feature;
mod hosted;
mod hosted_workflow;
mod machine;
mod run;
mod scoring;

pub use ai_analysis::{
    PROJECT_AI_ANALYSIS_CONTRACT, PROJECT_AI_ANALYSIS_SCHEMA_VERSION, ProjectAiAnalysisEnvelope,
    StoredProjectAiAnalysis,
};
pub use classification::{
    ClassificationError, ClassificationErrorKind, ClassificationOutcome, ClassificationPolicy,
    MaturityObservation, MaturitySignal, TypeObservation, TypeSignal, classify_project,
    criteria_applicability,
};
pub use comparison::{
    Candidate, CandidateDescriptor, CandidateSearch, CandidateSearchError, CandidateSearchOutcome,
    CohortComparison, CohortMode, CohortQuery, ComparisonError, ComparisonErrorKind,
    ComparisonPolicy, ComparisonProfile, SearchDepth, SeedProject, discover_cohort,
};
pub use contract::validate_project_bundle_consistency;
pub use evidence::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord, ClassificationEvidenceFact,
    ClassificationEvidenceKindRecord, ClassificationEvidenceRecord, ClassificationTagRecord,
    ClassifiedSnapshotFile, EvidenceAssemblyError, EvidenceAssemblyErrorKind, EvidenceSourceRecord,
    GitEvidenceProvenance, GitObjectFormatRecord, HistoryScopeEvidence, ParentDeltaEvidence,
    PortablePathEncoding, PortableRepositoryPath, ProjectEvidenceManifest, RawEvidenceFact,
    RawEvidenceIssue, RawEvidenceKind, RawEvidencePayload, TrackedFileEvidence,
    assemble_project_evidence,
};
pub use hosted::{
    HOSTED_API_CONTRACT, HOSTED_API_SCHEMA_VERSION, HostedAdmission, HostedContractEnvelope,
    HostedContractValueError, HostedErrorResponse, HostedEvaluationStatus, HostedJobStage,
    HostedJobState, HostedProjectStatus, HostedProjectStatusRecord, HostedRecentSourceStatus,
    HostedRecentSourceStatusRecord, HostedRequestState, HostedScoreStatus, HostedSubmission,
    HostedSubmissionRequest,
};
pub use hosted_workflow::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedEvaluationPort,
    HostedFailure, HostedFailureScope, HostedPortError, HostedPortErrorKind, HostedScoreArtifact,
    HostedSourceCollection, HostedSourceCollectionPort, HostedStoredSource, HostedWorkflow,
    HostedWorkflowOutcome, HostedWorkflowPolicy, HostedWorkflowStage, HostedWorkflowStore,
};
pub use machine::{MachineMappingError, build_project_analysis};
pub use run::{
    AdminAction, AdminAuditEvent, Administrator, AttemptDisposition, PIPELINE_STAGES, ProjectRun,
    RetryPolicy, RunError, RunErrorKind, RunId, RunLifecycle, Stage, StageAttempt, StageStatus,
};
pub use scoring::{
    ASSAY_SCORE_DIMENSIONS, AssayScore, CitedStatement, CompiledEvaluation, CompilerPolicy,
    ContributionSource, DeterministicContribution, DimensionScore, EvaluatorDescriptor,
    EvaluatorProvider, PotentialContext, PotentialScore, ProjectClassification, ProjectMaturity,
    ProjectType, ScoreCompileError, ScoreCompileErrorKind, ScoreCompilerInput, ScoreContribution,
    ScoreDimension, Visibility,
};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
