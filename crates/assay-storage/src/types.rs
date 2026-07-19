use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct PublicAdmissionLimits {
    pub max_active: i64,
    pub completed_cooldown_seconds: i64,
    pub failure_backoff_seconds: i64,
    pub bucket_window_seconds: i64,
    pub bucket_cooldown_seconds: i64,
    pub anonymous_burst: i32,
    pub owner_burst: i32,
    pub provider_burst: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct FailureSettlement<'a> {
    pub stage: &'a str,
    pub code: &'a str,
    pub retryable: bool,
    pub requested_delay_seconds: Option<i64>,
    pub retry_delay_cap_seconds: i64,
    pub failure_circuit_threshold: i32,
    pub failure_circuit_cooldown_seconds: i64,
    pub provider_failure: bool,
}

#[derive(Clone, Debug)]
pub struct ClaimedJob {
    pub job_id: Uuid,
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub generation: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub lease_generation: i64,
    pub lease_token: Uuid,
    pub stage: String,
    pub source_snapshot_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct GitHubCollection {
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

#[derive(Clone, Debug)]
pub struct StoredSource {
    pub source_snapshot_id: Uuid,
    pub source_observation_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct StoredEvaluationInput {
    pub source: StoredSource,
    pub normalized_facts: Value,
}

#[derive(Clone, Deserialize)]
pub struct EvaluationAttempt {
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
    pub judgment: Option<Value>,
}

impl std::fmt::Debug for EvaluationAttempt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EvaluationAttempt")
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field("evaluator_profile", &self.evaluator_profile)
            .field("rubric_version", &self.rubric_version)
            .field("prompt_version", &self.prompt_version)
            .field("evaluation_version", &self.evaluation_version)
            .field("provider_profile_version", &self.provider_profile_version)
            .field("sampling", &self.sampling)
            .field("evidence_bundle_hash", &self.evidence_bundle_hash)
            .field("usage", &self.usage)
            .field("latency_ms", &self.latency_ms)
            .field("status", &self.status)
            .field("error_code", &self.error_code)
            .field(
                "judgment",
                &self.judgment.as_ref().map(|_| "<validated-ai-judgment>"),
            )
            .finish()
    }
}
