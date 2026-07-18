//! Provider-independent orchestration for the hosted source workflow.
//!
//! This module owns sequencing and retry policy, while concrete database,
//! GitHub, and model-provider I/O remains behind ports implemented by adapter
//! crates.

#![allow(async_fn_in_trait)]

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

pub trait HostedWorkflowStore: Sync {
    async fn claim_job(&self, worker: &str) -> Result<Option<HostedClaimedJob>, HostedPortError>;

    async fn load_evaluation_input(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<Option<HostedEvaluationInput>, HostedPortError>;

    async fn store_source_collection(
        &self,
        job: &HostedClaimedJob,
        collection: &HostedSourceCollection,
    ) -> Result<HostedStoredSource, HostedPortError>;

    async fn record_evaluation_attempt(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError>;

    async fn store_evaluation(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError>;

    async fn settle_failure(
        &self,
        job: &HostedClaimedJob,
        stage: HostedWorkflowStage,
        failure: &HostedFailure,
        policy: HostedWorkflowPolicy,
    ) -> Result<bool, HostedPortError>;
}

pub trait HostedSourceCollectionPort: Sync {
    async fn collect(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<HostedSourceCollection, HostedFailure>;
}

pub trait HostedEvaluationPort: Sync {
    async fn evaluate(
        &self,
        input: &HostedEvaluationInput,
    ) -> Result<HostedEvaluationAttempt, HostedFailure>;
}

pub struct HostedWorkflow<'a, S, C, E> {
    store: &'a S,
    collector: &'a C,
    evaluator: &'a E,
    policy: HostedWorkflowPolicy,
}

impl<'a, S, C, E> HostedWorkflow<'a, S, C, E>
where
    S: HostedWorkflowStore,
    C: HostedSourceCollectionPort,
    E: HostedEvaluationPort,
{
    pub const fn new(
        store: &'a S,
        collector: &'a C,
        evaluator: &'a E,
        policy: HostedWorkflowPolicy,
    ) -> Self {
        Self {
            store,
            collector,
            evaluator,
            policy,
        }
    }

    pub async fn run_once(
        &self,
        worker_id: &str,
    ) -> Result<HostedWorkflowOutcome, HostedPortError> {
        let Some(job) = self.store.claim_job(worker_id).await? else {
            return Ok(HostedWorkflowOutcome::Idle);
        };
        self.process(job).await.or_else(|error| {
            if error.kind() == HostedPortErrorKind::LeaseLost {
                Ok(HostedWorkflowOutcome::LeaseLost)
            } else {
                Err(error)
            }
        })
    }

    async fn process(
        &self,
        job: HostedClaimedJob,
    ) -> Result<HostedWorkflowOutcome, HostedPortError> {
        let input = if job.stage == HostedWorkflowStage::Evaluating {
            match self.store.load_evaluation_input(&job).await? {
                Some(input) => input,
                None => {
                    return self
                        .settle(
                            &job,
                            HostedWorkflowStage::Evaluating,
                            HostedFailure::new("source_snapshot_missing", false),
                        )
                        .await;
                }
            }
        } else {
            let collection = match self.collector.collect(&job).await {
                Ok(collection) => collection,
                Err(failure) => {
                    return self
                        .settle(&job, HostedWorkflowStage::Collecting, failure)
                        .await;
                }
            };
            let facts = collection.normalized_facts.clone();
            let source = self
                .store
                .store_source_collection(&job, &collection)
                .await?;
            HostedEvaluationInput {
                source,
                normalized_facts: facts,
            }
        };

        let evaluation = match self.evaluator.evaluate(&input).await {
            Ok(evaluation) => evaluation,
            Err(failure) => {
                if let Some(attempt) = failure.evaluation_attempt.as_deref() {
                    self.store
                        .record_evaluation_attempt(&job, &input.source, attempt)
                        .await?;
                }
                return self
                    .settle(&job, HostedWorkflowStage::Evaluating, failure)
                    .await;
            }
        };
        self.store
            .store_evaluation(&job, &input.source, &evaluation)
            .await?;
        Ok(HostedWorkflowOutcome::Complete)
    }

    async fn settle(
        &self,
        job: &HostedClaimedJob,
        stage: HostedWorkflowStage,
        failure: HostedFailure,
    ) -> Result<HostedWorkflowOutcome, HostedPortError> {
        if self
            .store
            .settle_failure(job, stage, &failure, self.policy)
            .await?
        {
            Ok(HostedWorkflowOutcome::RetryScheduled)
        } else {
            Ok(HostedWorkflowOutcome::TerminalUnavailable)
        }
    }
}
