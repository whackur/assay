use assay_project_intelligence::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedFailure,
    HostedFailureScope, HostedPortError, HostedSourceCollection, HostedStoredSource,
    HostedWorkflowPolicy, HostedWorkflowStage, HostedWorkflowStore,
};

use crate::error::StorageError;
use crate::storage::Storage;
use crate::types::{
    ClaimedJob, EvaluationAttempt, FailureSettlement, GitHubCollection, StoredSource,
};

impl HostedWorkflowStore for Storage {
    async fn claim_job(&self, worker: &str) -> Result<Option<HostedClaimedJob>, HostedPortError> {
        Storage::claim_job(self, worker)
            .await
            .map(|job| job.map(workflow_job))
            .map_err(workflow_error)
    }

    async fn load_evaluation_input(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<Option<HostedEvaluationInput>, HostedPortError> {
        Storage::load_evaluation_input(self, &storage_job(job))
            .await
            .map(|input| {
                input.map(|input| HostedEvaluationInput {
                    source: workflow_source(&input.source),
                    normalized_facts: input.normalized_facts,
                })
            })
            .map_err(workflow_error)
    }

    async fn store_source_collection(
        &self,
        job: &HostedClaimedJob,
        collection: &HostedSourceCollection,
    ) -> Result<HostedStoredSource, HostedPortError> {
        let collection = GitHubCollection {
            provider_repository_id: collection.provider_repository_id,
            owner: collection.owner.clone(),
            repository: collection.repository.clone(),
            canonical_url: collection.canonical_url.clone(),
            default_branch: collection.default_branch.clone(),
            head_sha: collection.head_sha.clone(),
            source_url: collection.source_url.clone(),
            etag: collection.etag.clone(),
            normalized_facts: collection.normalized_facts.clone(),
        };
        Storage::store_github_collection(self, &storage_job(job), &collection)
            .await
            .map(|source| workflow_source(&source))
            .map_err(workflow_error)
    }

    async fn record_evaluation_attempt(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError> {
        Storage::record_evaluation_attempt(
            self,
            &storage_job(job),
            &storage_source(source),
            &storage_attempt(attempt),
        )
        .await
        .map(|_| ())
        .map_err(workflow_error)
    }

    async fn store_evaluation(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError> {
        Storage::store_evaluation(
            self,
            &storage_job(job),
            &storage_source(source),
            &storage_attempt(attempt),
        )
        .await
        .map(|_| ())
        .map_err(workflow_error)
    }

    async fn settle_failure(
        &self,
        job: &HostedClaimedJob,
        stage: HostedWorkflowStage,
        failure: &HostedFailure,
        policy: HostedWorkflowPolicy,
    ) -> Result<bool, HostedPortError> {
        Storage::settle_failure(
            self,
            &storage_job(job),
            FailureSettlement {
                stage: stage.as_str(),
                code: &failure.code,
                retryable: failure.retryable,
                requested_delay_seconds: failure.retry_after_seconds,
                retry_delay_cap_seconds: policy.retry_delay_cap_seconds,
                failure_circuit_threshold: policy.failure_circuit_threshold,
                failure_circuit_cooldown_seconds: policy.failure_circuit_cooldown_seconds,
                provider_failure: failure.scope == HostedFailureScope::Provider,
            },
        )
        .await
        .map_err(workflow_error)
    }
}

fn workflow_error(error: StorageError) -> HostedPortError {
    match error {
        StorageError::LeaseLost => HostedPortError::lease_lost(),
        StorageError::Database(_) => HostedPortError::unavailable(),
    }
}

fn workflow_job(job: ClaimedJob) -> HostedClaimedJob {
    HostedClaimedJob {
        job_id: job.job_id,
        request_id: job.request_id,
        owner: job.owner,
        repository: job.repository,
        generation: job.generation,
        attempt_count: job.attempt_count,
        max_attempts: job.max_attempts,
        lease_generation: job.lease_generation,
        lease_token: job.lease_token,
        stage: if job.stage == "evaluating" {
            HostedWorkflowStage::Evaluating
        } else {
            HostedWorkflowStage::Collecting
        },
        source_snapshot_id: job.source_snapshot_id,
    }
}

fn storage_job(job: &HostedClaimedJob) -> ClaimedJob {
    ClaimedJob {
        job_id: job.job_id,
        request_id: job.request_id,
        owner: job.owner.clone(),
        repository: job.repository.clone(),
        generation: job.generation,
        attempt_count: job.attempt_count,
        max_attempts: job.max_attempts,
        lease_generation: job.lease_generation,
        lease_token: job.lease_token,
        stage: job.stage.as_str().to_owned(),
        source_snapshot_id: job.source_snapshot_id,
    }
}

fn workflow_source(source: &StoredSource) -> HostedStoredSource {
    HostedStoredSource {
        source_snapshot_id: source.source_snapshot_id,
        source_observation_id: source.source_observation_id,
    }
}

fn storage_source(source: &HostedStoredSource) -> StoredSource {
    StoredSource {
        source_snapshot_id: source.source_snapshot_id,
        source_observation_id: source.source_observation_id,
    }
}

fn storage_attempt(attempt: &HostedEvaluationAttempt) -> EvaluationAttempt {
    EvaluationAttempt {
        provider_id: attempt.provider_id.clone(),
        model: attempt.model.clone(),
        evaluator_profile: attempt.evaluator_profile.clone(),
        rubric_version: attempt.rubric_version.clone(),
        prompt_version: attempt.prompt_version.clone(),
        evaluation_version: attempt.evaluation_version.clone(),
        provider_profile_version: attempt.provider_profile_version.clone(),
        sampling: attempt.sampling.clone(),
        evidence_bundle_hash: attempt.evidence_bundle_hash.clone(),
        usage: attempt.usage.clone(),
        latency_ms: attempt.latency_ms,
        status: attempt.status.clone(),
        error_code: attempt.error_code.clone(),
    }
}
