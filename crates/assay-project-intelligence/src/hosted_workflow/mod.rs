//! Provider-independent orchestration for the hosted source workflow.
//!
//! This module owns sequencing and retry policy, while concrete database,
//! GitHub, and model-provider I/O remains behind ports implemented by adapter
//! crates.

#![allow(async_fn_in_trait)]

mod ports;
mod types;

pub use ports::{HostedEvaluationPort, HostedSourceCollectionPort, HostedWorkflowStore};
pub use types::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedFailure,
    HostedFailureScope, HostedPortError, HostedPortErrorKind, HostedSourceCollection,
    HostedStoredSource, HostedWorkflowOutcome, HostedWorkflowPolicy, HostedWorkflowStage,
};

// Trait method calls are resolved through the bounds on `HostedWorkflow`'s
// `where` clause; no separate trait-import statements are needed in edition 2024.

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
