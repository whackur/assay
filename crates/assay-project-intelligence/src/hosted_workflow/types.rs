use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedWorkflowStage {
    Collecting,
    Evaluating,
}

impl HostedWorkflowStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Collecting => "collecting",
            Self::Evaluating => "evaluating",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedClaimedJob {
    pub job_id: Uuid,
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub generation: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub lease_generation: i64,
    pub lease_token: Uuid,
    pub stage: HostedWorkflowStage,
    pub source_snapshot_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct HostedSourceCollection {
    pub provider_repository_id: i64,
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub default_branch: String,
    pub head_sha: String,
    pub source_url: String,
    pub etag: Option<String>,
    pub normalized_facts: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedStoredSource {
    pub source_snapshot_id: Uuid,
    pub source_observation_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct HostedEvaluationInput {
    pub source: HostedStoredSource,
    pub normalized_facts: Value,
}

#[derive(Clone, Debug)]
pub struct HostedEvaluationAttempt {
    pub provider_id: String,
    pub model: String,
    pub evaluator_profile: String,
    pub rubric_version: String,
    pub prompt_version: String,
    pub evaluation_version: String,
    pub provider_profile_version: String,
    pub sampling: Value,
    pub evidence_bundle_hash: String,
    pub usage: Option<Value>,
    pub latency_ms: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HostedFailure {
    pub code: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<i64>,
    pub evaluation_attempt: Option<Box<HostedEvaluationAttempt>>,
    pub scope: HostedFailureScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedFailureScope {
    Repository,
    Provider,
}

impl HostedFailure {
    pub fn new(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            retryable,
            retry_after_seconds: None,
            evaluation_attempt: None,
            scope: HostedFailureScope::Repository,
        }
    }

    pub fn provider(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            scope: HostedFailureScope::Provider,
            ..Self::new(code, retryable)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedPortErrorKind {
    LeaseLost,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedPortError {
    kind: HostedPortErrorKind,
}

impl HostedPortError {
    pub const fn lease_lost() -> Self {
        Self {
            kind: HostedPortErrorKind::LeaseLost,
        }
    }

    pub const fn unavailable() -> Self {
        Self {
            kind: HostedPortErrorKind::Unavailable,
        }
    }

    pub const fn kind(self) -> HostedPortErrorKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedWorkflowPolicy {
    pub retry_delay_cap_seconds: i64,
    pub failure_circuit_threshold: i32,
    pub failure_circuit_cooldown_seconds: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedWorkflowOutcome {
    Idle,
    Complete,
    RetryScheduled,
    TerminalUnavailable,
    LeaseLost,
}
