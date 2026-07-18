//! Versioned, evidence-bounded qualitative evaluation for Assay projects.
//!
//! Provider output and provider prose are untrusted until [`Evaluator`]
//! validates their schema-shaped fields, rubric membership, bounded ratings,
//! and citations against the exact [`EvidenceBundle`]. The validated scoring
//! view intentionally excludes provider rationale. Two provider families plug
//! into the same boundary (ADR 0012): API-key HTTP adapters built on the
//! shared [`ApiKeyEvaluator`] machinery over the injected [`SecretStore`] and
//! [`HttpTransport`] ports, and agentic CLI adapters built on the injected
//! [`SnapshotWorkspace`] and [`AgentRunner`] ports. The crate performs no
//! network, filesystem, process, credential, or score-compilation I/O; every
//! OpenAI keeps its HTTP client injected, while the hosted Ollama-compatible
//! profile also provides a bounded concrete HTTP transport in this crate so
//! protocol behavior does not leak into the worker. Secret stores, snapshot
//! materializers, and process runners remain injected, and every provider's
//! untrusted bytes pass through the one [`Evaluator`] validation path.

#![forbid(unsafe_code)]

mod agentic;
mod api;
mod bundle;
mod error;
mod evaluator;
mod ollama;
mod openai;
mod rubric;

pub use agentic::{
    AGENT_INSTRUCTIONS, AgentIdentity, AgentRun, AgentRunError, AgentRunner, AgenticConfig,
    AgenticEvaluator, AgenticProvenance, AgenticSnapshot, ControlInputs, PreparedWorkspace,
    SnapshotWorkspace, WorkspaceError,
};
pub use api::{
    ApiKeyEvaluator, ApiProviderProfile, AuthorizationScheme, EvaluationSnapshot, HttpTransport,
    OutboundRequest, ProviderReply, ProviderSecret, ProviderTelemetry, SamplingConfig, SecretError,
    SecretName, SecretStore, SnapshotOutcome, SnapshotProvenance, TransportError,
    TransportResponse, Usage,
};
pub use bundle::{
    EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope, ExternalTransmission,
    TransmissionSurface,
};
pub use error::{EvaluationError, EvaluationErrorKind, ProviderError};
pub use evaluator::{
    Applicability, DeterministicFakeProvider, EvaluationProvider, EvaluationStatus, Evaluator,
    ProviderExecutionBoundary, ProviderRequest, ScoringJudgment, ValidatedJudgment,
    ValidatedJudgmentSet,
};
pub use ollama::{
    HostedOllamaWorkflowEvaluator, OLLAMA_COMPATIBLE_PROFILE, OLLAMA_COMPATIBLE_PROVIDER_ID,
    OllamaCompatibleConfig, OllamaCompatibleEvaluator, OllamaCompatibleHttpTransport,
    OllamaConfigError, OllamaFailureDisposition, OllamaProfile, build_hosted_metadata_bundle,
    classify_ollama_failure,
};
pub use openai::{OpenAiConfig, OpenAiEvaluator, OpenAiProfile};
pub use rubric::{QualitativeCriterion, QualitativeRubric};

/// Stable public schema version produced by validated evaluator results.
pub const AI_JUDGMENT_SCHEMA_VERSION: &str = "1.0.0";

/// Stable evaluation version required by the initial project rubric.
pub const PROJECT_EVALUATION_VERSION: &str = "project-intelligence-1";

/// Stable prompt-envelope version shared by provider adapters.
pub const PROMPT_VERSION: &str = "project-evaluation-prompt-1";
