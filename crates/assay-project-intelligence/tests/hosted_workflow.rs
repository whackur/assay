use std::str::FromStr;
use std::sync::Mutex;

use assay_domain::{
    AnalysisVersion, ContentHash, EvidenceStatus, RepositorySource, RevisionId,
    RubricApplicability, RubricCriterionId, RubricJudgment, RubricJudgmentSet,
};
use assay_project_intelligence::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedEvaluationPort,
    HostedFailure, HostedPortError, HostedSourceCollection, HostedSourceCollectionPort,
    HostedStoredSource, HostedWorkflow, HostedWorkflowOutcome, HostedWorkflowPolicy,
    HostedWorkflowStage, HostedWorkflowStore,
};
use serde_json::json;
use uuid::Uuid;

struct FakeStore {
    job: Mutex<Option<HostedClaimedJob>>,
    input: Option<HostedEvaluationInput>,
    recorded_attempts: Mutex<usize>,
    recorded_attempt: Mutex<Option<HostedEvaluationAttempt>>,
    stored_evaluations: Mutex<usize>,
    stored_attempt: Mutex<Option<HostedEvaluationAttempt>>,
    stored_score: Mutex<Option<assay_project_intelligence::HostedScoreArtifact>>,
    settled: Mutex<usize>,
}

impl HostedWorkflowStore for FakeStore {
    async fn claim_job(&self, _: &str) -> Result<Option<HostedClaimedJob>, HostedPortError> {
        Ok(self.job.lock().unwrap().take())
    }

    async fn load_evaluation_input(
        &self,
        _: &HostedClaimedJob,
    ) -> Result<Option<HostedEvaluationInput>, HostedPortError> {
        Ok(self.input.clone())
    }

    async fn store_source_collection(
        &self,
        _: &HostedClaimedJob,
        _: &HostedSourceCollection,
    ) -> Result<HostedStoredSource, HostedPortError> {
        Ok(source())
    }

    async fn record_evaluation_attempt(
        &self,
        _: &HostedClaimedJob,
        _: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError> {
        *self.recorded_attempts.lock().unwrap() += 1;
        *self.recorded_attempt.lock().unwrap() = Some(attempt.clone());
        Ok(())
    }

    async fn store_evaluation(
        &self,
        _: &HostedClaimedJob,
        _: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
        score: &assay_project_intelligence::HostedScoreArtifact,
    ) -> Result<(), HostedPortError> {
        *self.stored_evaluations.lock().unwrap() += 1;
        *self.stored_attempt.lock().unwrap() = Some(attempt.clone());
        *self.stored_score.lock().unwrap() = Some(score.clone());
        Ok(())
    }

    async fn settle_failure(
        &self,
        _: &HostedClaimedJob,
        _: HostedWorkflowStage,
        _: &HostedFailure,
        _: HostedWorkflowPolicy,
    ) -> Result<bool, HostedPortError> {
        *self.settled.lock().unwrap() += 1;
        Ok(true)
    }
}

struct CountingCollector(Mutex<usize>);

impl HostedSourceCollectionPort for CountingCollector {
    async fn collect(&self, _: &HostedClaimedJob) -> Result<HostedSourceCollection, HostedFailure> {
        *self.0.lock().unwrap() += 1;
        Ok(HostedSourceCollection {
            provider_repository_id: 42,
            owner: "whackur".to_owned(),
            repository: "assay".to_owned(),
            canonical_url: "https://github.com/whackur/assay".to_owned(),
            default_branch: "main".to_owned(),
            head_sha: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            source_url: "https://api.github.com/repos/whackur/assay".to_owned(),
            etag: None,
            normalized_facts: json!({"stargazers_count": 1}),
        })
    }
}

struct RetryingEvaluator;

impl HostedEvaluationPort for RetryingEvaluator {
    async fn evaluate(
        &self,
        _: &HostedEvaluationInput,
    ) -> Result<HostedEvaluationAttempt, HostedFailure> {
        let mut failure = HostedFailure::new("ollama_timeout", true);
        failure.evaluation_attempt = Some(Box::new(attempt("partial")));
        Err(failure)
    }
}

struct SuccessfulEvaluator;

impl HostedEvaluationPort for SuccessfulEvaluator {
    async fn evaluate(
        &self,
        _: &HostedEvaluationInput,
    ) -> Result<HostedEvaluationAttempt, HostedFailure> {
        Ok(attempt("validated_unpublished"))
    }
}

fn job(stage: HostedWorkflowStage) -> HostedClaimedJob {
    HostedClaimedJob {
        job_id: Uuid::nil(),
        request_id: Uuid::nil(),
        owner: "whackur".to_owned(),
        repository: "assay".to_owned(),
        generation: 1,
        attempt_count: 1,
        max_attempts: 5,
        lease_generation: 1,
        lease_token: Uuid::nil(),
        stage,
        source_snapshot_id: Some(Uuid::nil()),
    }
}

fn source() -> HostedStoredSource {
    HostedStoredSource {
        source_snapshot_id: Uuid::nil(),
        source_observation_id: Uuid::nil(),
    }
}

fn attempt(status: &str) -> HostedEvaluationAttempt {
    HostedEvaluationAttempt {
        provider_id: "ollama-openai-compatible-api-1".to_owned(),
        model: "qwen3".to_owned(),
        evaluator_profile: "ollama-compatible-1".to_owned(),
        rubric_version: "project-rubric-1".to_owned(),
        prompt_version: "prompt-1".to_owned(),
        evaluation_version: "evaluation-1".to_owned(),
        provider_profile_version: "profile-1".to_owned(),
        sampling: json!({}),
        evidence_bundle_hash:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        usage: None,
        latency_ms: None,
        status: status.to_owned(),
        error_code: None,
        judgment: (status == "validated_unpublished").then(valid_judgment),
        validated_judgments: (status == "validated_unpublished").then(valid_judgment_set),
    }
}

fn valid_judgment_set() -> assay_domain::RubricJudgmentSet {
    RubricJudgmentSet::new(
        AnalysisVersion::from_str("project-intelligence-1").unwrap(),
        AnalysisVersion::from_str("project-rubric-1").unwrap(),
        EvidenceStatus::Complete,
        ContentHash::from_str(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        vec![
            RubricJudgment::new(
                RubricCriterionId::from_str("maintenance_health.maintainability").unwrap(),
                RubricApplicability::Applicable,
                Some(4),
                4,
                0.9,
                vec![
                    assay_domain::EvidenceId::from_str("evidence:repository_feature:fixture")
                        .unwrap(),
                ],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn valid_judgment() -> serde_json::Value {
    json!({
        "schema_version": "1.0.0",
        "evaluation_version": "evaluation-1",
        "rubric_version": "project-rubric-1",
        "status": "complete",
        "evidence_bundle_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "privacy": {
            "evidence_scope": "public_only",
            "external_transmission": "public_only"
        },
        "judgments": [{
            "criterion_id": "project.maintainability",
            "applicability": "applicable",
            "rating": 4,
            "rating_scale": 4,
            "confidence": 0.9,
            "evidence_ids": ["evidence:repository_feature:fixture"],
            "rationale": "The cited public evidence supports this rating."
        }]
    })
}

fn policy() -> HostedWorkflowPolicy {
    HostedWorkflowPolicy {
        retry_delay_cap_seconds: 3_600,
        failure_circuit_threshold: 3,
        failure_circuit_cooldown_seconds: 900,
    }
}

#[tokio::test]
async fn evaluation_retry_reuses_durable_source_and_preserves_attempt() {
    let store = FakeStore {
        job: Mutex::new(Some(job(HostedWorkflowStage::Evaluating))),
        input: Some(HostedEvaluationInput {
            source: source(),
            project_source: RepositorySource::hosted("github", "whackur", "assay").unwrap(),
            revision: RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
            normalized_facts: json!({"stargazers_count": 1}),
        }),
        recorded_attempts: Mutex::new(0),
        recorded_attempt: Mutex::new(None),
        stored_evaluations: Mutex::new(0),
        stored_attempt: Mutex::new(None),
        stored_score: Mutex::new(None),
        settled: Mutex::new(0),
    };
    let collector = CountingCollector(Mutex::new(0));
    let workflow = HostedWorkflow::new(&store, &collector, &RetryingEvaluator, policy());

    assert_eq!(
        workflow.run_once("worker").await.unwrap(),
        HostedWorkflowOutcome::RetryScheduled
    );
    assert_eq!(*collector.0.lock().unwrap(), 0);
    assert_eq!(*store.recorded_attempts.lock().unwrap(), 1);
    assert_eq!(
        store
            .recorded_attempt
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|attempt| attempt.judgment.as_ref()),
        None
    );
    assert_eq!(*store.settled.lock().unwrap(), 1);
}

#[tokio::test]
async fn collecting_sequence_persists_source_before_validated_unpublished_evaluation() {
    let store = FakeStore {
        job: Mutex::new(Some(job(HostedWorkflowStage::Collecting))),
        input: None,
        recorded_attempts: Mutex::new(0),
        recorded_attempt: Mutex::new(None),
        stored_evaluations: Mutex::new(0),
        stored_attempt: Mutex::new(None),
        stored_score: Mutex::new(None),
        settled: Mutex::new(0),
    };
    let collector = CountingCollector(Mutex::new(0));
    let workflow = HostedWorkflow::new(&store, &collector, &SuccessfulEvaluator, policy());

    assert_eq!(
        workflow.run_once("worker").await.unwrap(),
        HostedWorkflowOutcome::Complete
    );
    assert_eq!(*collector.0.lock().unwrap(), 1);
    assert_eq!(*store.stored_evaluations.lock().unwrap(), 1);
    let judgment = store
        .stored_attempt
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|attempt| attempt.judgment.clone());
    assert_eq!(judgment.as_ref(), Some(&valid_judgment()));
    assert_eq!(
        judgment
            .as_ref()
            .and_then(|value| value["judgments"][0]["rationale"].as_str()),
        Some("The cited public evidence supports this rating.")
    );
    let score = store.stored_score.lock().unwrap().clone().unwrap();
    assert_eq!(score.status, "insufficient");
    assert_eq!(score.value, None);
    assert!(!score.snapshot.to_string().contains("rationale"));
    assert_eq!(score.snapshot["evaluator"]["provider"], "ollama_compatible");
}
