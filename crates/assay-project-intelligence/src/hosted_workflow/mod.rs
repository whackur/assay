//! Provider-independent orchestration for the hosted source workflow.
//!
//! This module owns sequencing and retry policy, while concrete database,
//! GitHub, and model-provider I/O remains behind ports implemented by adapter
//! crates.

#![allow(async_fn_in_trait)]

use std::str::FromStr;

mod ports;
mod types;

pub use ports::{HostedEvaluationPort, HostedSourceCollectionPort, HostedWorkflowStore};
pub use types::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedFailure,
    HostedFailureScope, HostedPortError, HostedPortErrorKind, HostedScoreArtifact,
    HostedSourceCollection, HostedStoredSource, HostedWorkflowOutcome, HostedWorkflowPolicy,
    HostedWorkflowStage,
};

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
                project_source: assay_domain::RepositorySource::hosted(
                    "github",
                    &collection.owner,
                    &collection.repository,
                )
                .map_err(|_| HostedPortError::unavailable())?,
                revision: assay_domain::RevisionId::from_str(&collection.head_sha)
                    .map_err(|_| HostedPortError::unavailable())?,
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
        let Some(judgments) = evaluation.validated_judgments.clone() else {
            if evaluation.status != "validated_unpublished" {
                self.store
                    .store_evaluation(
                        &job,
                        &input.source,
                        &evaluation,
                        &HostedScoreArtifact {
                            status: "unavailable".to_owned(),
                            value: None,
                            snapshot: serde_json::json!({}),
                        },
                    )
                    .await?;
                return Ok(HostedWorkflowOutcome::Complete);
            }
            return self
                .settle(
                    &job,
                    HostedWorkflowStage::Evaluating,
                    HostedFailure::new("score_compilation_input_missing", false),
                )
                .await;
        };
        let evaluator = crate::EvaluatorDescriptor::new(
            &evaluation.evaluator_profile,
            crate::EvaluatorProvider::OllamaCompatible,
            Some(&evaluation.model),
            &evaluation.rubric_version,
        )
        .map_err(|_| HostedPortError::unavailable())?;
        let classification = crate::ProjectClassification::new(
            assay_domain::EvidenceStatus::Unavailable,
            None,
            Vec::new(),
            Vec::new(),
            None,
            0.0,
            Vec::new(),
        )
        .map_err(|_| HostedPortError::unavailable())?;
        let compiled = crate::ScoreCompilerInput::new(
            input.project_source.clone(),
            input.revision.clone(),
            evaluator,
            crate::Visibility::Public,
            classification,
            Vec::new(),
            Some(judgments),
            crate::PotentialContext::default(),
            crate::CompilerPolicy::v1(),
        )
        .compile()
        .map_err(|_| HostedPortError::unavailable())?;
        let score = HostedScoreArtifact {
            status: score_status(
                compiled.assay_score().status(),
                compiled.assay_score().value(),
            )
            .to_owned(),
            value: compiled.assay_score().value(),
            snapshot: compiled.to_machine_value(),
        };
        self.store
            .store_evaluation(&job, &input.source, &evaluation, &score)
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

fn score_status(status: assay_domain::EvidenceStatus, value: Option<f64>) -> &'static str {
    match (status, value.is_some()) {
        (assay_domain::EvidenceStatus::Complete, true) => "complete",
        (assay_domain::EvidenceStatus::Partial, true) => "partial",
        (assay_domain::EvidenceStatus::Insufficient, _) => "insufficient",
        (assay_domain::EvidenceStatus::Complete | assay_domain::EvidenceStatus::Partial, false) => {
            "insufficient"
        }
        (assay_domain::EvidenceStatus::Unavailable, _) => "unavailable",
        _ => "unavailable",
    }
}
