//! Contract tests for the agentic CLI family over deterministic port fakes.
//!
//! No process, network, or filesystem I/O happens here: a fake
//! `SnapshotWorkspace` records the control inputs it was asked to write and a
//! fake `AgentRunner` returns scripted untrusted bytes, exactly as the
//! scripted transport exercises the API-key family.

use std::{
    cell::RefCell,
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use assay_ai_evaluator::{
    AgentIdentity, AgentRun, AgentRunError, AgentRunner, AgenticConfig, AgenticEvaluator,
    ControlInputs, EvaluationErrorKind, EvidenceBundle, EvidenceDescriptor, EvidenceKind,
    EvidenceScope, ExternalTransmission, PreparedWorkspace, ProviderExecutionBoundary,
    QualitativeRubric, SnapshotOutcome, SnapshotWorkspace, TransmissionSurface, WorkspaceError,
};
use assay_domain::EvidenceId;
use serde_json::{Value, json};

const ANALYZED_COMMIT: &str = "8b7df143d91c716ecfa5fc1730022f6b421b05cedee8fd52b1fc65a96030ad52";

fn evidence_id(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

fn items() -> Vec<EvidenceDescriptor> {
    vec![
        EvidenceDescriptor::new(
            evidence_id("evidence:readme:claim-4"),
            EvidenceKind::DocumentationClaim,
            "The README describes a repository analysis workflow.",
        )
        .unwrap(),
        EvidenceDescriptor::new(
            evidence_id("evidence:test:integration-2"),
            EvidenceKind::Test,
            "A cited integration test exercises the documented workflow.",
        )
        .unwrap(),
    ]
}

// A public-only bundle whose consent acknowledged the whole-snapshot surface.
fn snapshot_consented_bundle() -> EvidenceBundle {
    EvidenceBundle::with_acknowledged_surface(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        TransmissionSurface::WorktreeSnapshot,
        items(),
    )
    .unwrap()
}

// A public-only bundle whose consent acknowledged only the bundle facts.
fn bundle_only_consent_bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items(),
    )
    .unwrap()
}

fn judgment_bytes(bundle: &EvidenceBundle, mutator: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value = json!({
        "schema_version": "1.0.0",
        "evaluation_version": "project-intelligence-1",
        "rubric_version": "project-rubric-1",
        "status": "partial",
        "evidence_bundle_hash": bundle.content_hash(),
        "privacy": {
            "evidence_scope": "public_only",
            "external_transmission": "public_only"
        },
        "judgments": [{
            "criterion_id": "substance.claim_implementation_fit",
            "applicability": "applicable",
            "rating": 3,
            "rating_scale": 4,
            "confidence": 0.82,
            "evidence_ids": ["evidence:readme:claim-4", "evidence:test:integration-2"],
            "rationale": "The documented workflow is supported by cited implementation and test evidence."
        }]
    });
    mutator(&mut value);
    serde_json::to_vec(&value).unwrap()
}

struct FakeWorkspace {
    materialize_error: Option<WorkspaceError>,
    seen_commit: RefCell<Option<String>>,
    seen_evidence_ids: RefCell<Vec<String>>,
    seen_payload: RefCell<Option<String>>,
    seen_instructions: RefCell<Option<String>>,
    disposed: AtomicUsize,
}

impl FakeWorkspace {
    fn ready() -> Self {
        Self {
            materialize_error: None,
            seen_commit: RefCell::new(None),
            seen_evidence_ids: RefCell::new(Vec::new()),
            seen_payload: RefCell::new(None),
            seen_instructions: RefCell::new(None),
            disposed: AtomicUsize::new(0),
        }
    }

    fn failing(error: WorkspaceError) -> Self {
        Self {
            materialize_error: Some(error),
            ..Self::ready()
        }
    }

    fn disposed(&self) -> usize {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl SnapshotWorkspace for FakeWorkspace {
    fn materialize(&self, inputs: &ControlInputs<'_>) -> Result<PreparedWorkspace, WorkspaceError> {
        if let Some(error) = self.materialize_error {
            return Err(error);
        }
        *self.seen_commit.borrow_mut() = Some(inputs.analyzed_commit().to_owned());
        *self.seen_evidence_ids.borrow_mut() = inputs
            .evidence_ids()
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
        *self.seen_payload.borrow_mut() = Some(inputs.canonical_payload().to_owned());
        *self.seen_instructions.borrow_mut() = Some(inputs.instructions().to_owned());
        Ok(PreparedWorkspace::new(
            PathBuf::from("snapshot"),
            PathBuf::from("control"),
            PathBuf::from("control").join("judgment.json"),
        ))
    }

    fn dispose(&self, _workspace: PreparedWorkspace) {
        self.disposed.fetch_add(1, Ordering::SeqCst);
    }
}

struct FakeRunner {
    result: Result<Vec<u8>, AgentRunError>,
    probe_error: Option<AgentRunError>,
    calls: AtomicUsize,
}

impl FakeRunner {
    fn returning(judgment: Vec<u8>) -> Self {
        Self {
            result: Ok(judgment),
            probe_error: None,
            calls: AtomicUsize::new(0),
        }
    }

    fn failing(error: AgentRunError) -> Self {
        Self {
            result: Err(error),
            probe_error: None,
            calls: AtomicUsize::new(0),
        }
    }

    fn unprobeable() -> Self {
        Self {
            result: Err(AgentRunError::Failure),
            probe_error: Some(AgentRunError::ProbeFailed),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AgentRunner for FakeRunner {
    fn probe(&self) -> Result<AgentIdentity, AgentRunError> {
        match self.probe_error {
            Some(error) => Err(error),
            None => Ok(AgentIdentity::new("codex".to_owned(), "1.2.3".to_owned())),
        }
    }

    fn run(&self, workspace: &PreparedWorkspace) -> Result<AgentRun, AgentRunError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(workspace.snapshot_dir(), PathBuf::from("snapshot"));
        match &self.result {
            Ok(judgment) => Ok(AgentRun::new(judgment.clone(), "run-0001".to_owned())),
            Err(error) => Err(*error),
        }
    }
}

fn config() -> AgenticConfig {
    AgenticConfig {
        provider_id: "codex-cli-1",
        model: "gpt-5-codex".to_owned(),
        execution_boundary: ProviderExecutionBoundary::External,
        analyzed_commit: ANALYZED_COMMIT.to_owned(),
    }
}

fn adapter(
    workspace: FakeWorkspace,
    runner: FakeRunner,
) -> AgenticEvaluator<FakeWorkspace, FakeRunner> {
    AgenticEvaluator::new(QualitativeRubric::project_v1(), config(), workspace, runner)
}

#[test]
fn validated_agentic_judgment_carries_provider_and_run_provenance() {
    let bundle = snapshot_consented_bundle();
    let judgment = judgment_bytes(&bundle, |_| {});
    let adapter = adapter(FakeWorkspace::ready(), FakeRunner::returning(judgment));
    let snapshot = adapter.evaluate(&bundle);

    let SnapshotOutcome::Validated(set) = snapshot.outcome() else {
        panic!("expected a validated outcome, got {:?}", snapshot.outcome());
    };
    assert_eq!(set.evidence_bundle_hash(), bundle.content_hash());

    let provenance = snapshot.provenance();
    assert_eq!(provenance.provider_id(), "codex-cli-1");
    assert_eq!(provenance.analyzed_commit(), ANALYZED_COMMIT);
    assert_eq!(provenance.model(), "gpt-5-codex");
    let agent = provenance.agent().expect("probed agent identity");
    assert_eq!(agent.cli(), "codex");
    assert_eq!(agent.version(), "1.2.3");
    assert_eq!(provenance.run_id(), Some("run-0001"));
    assert_eq!(provenance.rubric_version(), "project-rubric-1");
    assert_eq!(provenance.evaluation_version(), "project-intelligence-1");
}

#[test]
fn control_inputs_carry_payload_commit_and_mandatory_evidence_list() {
    let bundle = snapshot_consented_bundle();
    let judgment = judgment_bytes(&bundle, |_| {});
    let adapter = adapter(FakeWorkspace::ready(), FakeRunner::returning(judgment));
    adapter.evaluate(&bundle);

    let workspace = adapter.workspace();
    assert_eq!(
        workspace.seen_commit.borrow().as_deref(),
        Some(ANALYZED_COMMIT)
    );
    assert_eq!(
        *workspace.seen_evidence_ids.borrow(),
        vec![
            "evidence:readme:claim-4".to_owned(),
            "evidence:test:integration-2".to_owned()
        ]
    );
    let payload = workspace.seen_payload.borrow().clone().unwrap();
    let payload: Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(payload["repository_evidence_is_untrusted_data"], true);
    assert_eq!(payload["evidence_bundle_hash"], bundle.content_hash());
    let instructions = workspace.seen_instructions.borrow().clone().unwrap();
    assert!(instructions.contains("untrusted data"));
    assert!(instructions.contains("listed evidence IDs"));
    assert_eq!(workspace.disposed(), 1);
}

#[test]
fn remote_agent_without_snapshot_consent_is_privacy_mismatch_and_never_runs() {
    let bundle = bundle_only_consent_bundle();
    let judgment = judgment_bytes(&bundle, |_| {});
    let adapter = adapter(FakeWorkspace::ready(), FakeRunner::returning(judgment));
    let snapshot = adapter.evaluate(&bundle);

    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::PrivacyMismatch)
    ));
    assert_eq!(snapshot.outcome().status_code(), "privacy_mismatch");
    assert!(snapshot.provenance().agent().is_none());
    assert!(snapshot.provenance().run_id().is_none());
    // Consent gating happens before any provider work starts.
    assert_eq!(adapter.runner().calls(), 0);
    assert!(adapter.workspace().seen_commit.borrow().is_none());
}

#[test]
fn local_endpoint_agent_requires_not_used_transmission() {
    // A fully local model endpoint may declare `Local`, which continues to
    // require `ExternalTransmission::NotUsed`.
    let bundle = EvidenceBundle::with_acknowledged_surface(
        EvidenceScope::PublicOnly,
        ExternalTransmission::NotUsed,
        TransmissionSurface::WorktreeSnapshot,
        items(),
    )
    .unwrap();
    let judgment = judgment_bytes(&bundle, |value| {
        value["privacy"]["external_transmission"] = json!("not_used");
    });
    let local_config = AgenticConfig {
        execution_boundary: ProviderExecutionBoundary::Local,
        ..config()
    };
    let adapter = AgenticEvaluator::new(
        QualitativeRubric::project_v1(),
        local_config,
        FakeWorkspace::ready(),
        FakeRunner::returning(judgment),
    );
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(snapshot.outcome(), SnapshotOutcome::Validated(_)));
}

#[test]
fn oversized_agent_output_is_rejected_by_the_shared_validator() {
    let bundle = snapshot_consented_bundle();
    let adapter = adapter(
        FakeWorkspace::ready(),
        FakeRunner::returning(vec![b' '; 64 * 1024 + 1]),
    );
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::OutputTooLarge)
    ));
}

#[test]
fn runner_output_bound_is_an_explicit_failure_not_a_fabricated_result() {
    let bundle = snapshot_consented_bundle();
    let adapter = adapter(
        FakeWorkspace::ready(),
        FakeRunner::failing(AgentRunError::OutputTooLarge),
    );
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::OutputTooLarge)
    ));
}

#[test]
fn malformed_agent_output_is_explicit() {
    let bundle = snapshot_consented_bundle();
    let adapter = adapter(
        FakeWorkspace::ready(),
        FakeRunner::returning(b"not-json".to_vec()),
    );
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::MalformedOutput)
    ));
}

#[test]
fn injected_instructions_cannot_smuggle_an_uncited_judgment_past_validation() {
    // A prompt-injected agent citing evidence outside the bundle is rejected
    // by the same validator every provider passes through.
    let bundle = snapshot_consented_bundle();
    let forged = judgment_bytes(&bundle, |value| {
        value["judgments"][0]["evidence_ids"] = json!(["evidence:source:forged"]);
    });
    let adapter = adapter(FakeWorkspace::ready(), FakeRunner::returning(forged));
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::UnknownEvidenceCitation)
    ));
}

#[test]
fn timeout_sandbox_violation_and_probe_failure_are_explicit_statuses() {
    let bundle = snapshot_consented_bundle();

    let timeout = adapter(
        FakeWorkspace::ready(),
        FakeRunner::failing(AgentRunError::Timeout),
    )
    .evaluate(&bundle);
    assert!(matches!(
        timeout.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderTimeout)
    ));
    assert!(timeout.provenance().run_id().is_none());

    let violation = adapter(
        FakeWorkspace::ready(),
        FakeRunner::failing(AgentRunError::SandboxViolation),
    )
    .evaluate(&bundle);
    assert!(matches!(
        violation.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::SandboxViolation)
    ));
    assert_eq!(violation.outcome().status_code(), "sandbox_violation");

    let unprobeable = FakeRunner::unprobeable();
    let probe_failed = adapter(FakeWorkspace::ready(), unprobeable).evaluate(&bundle);
    assert!(matches!(
        probe_failed.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderFailure)
    ));
    assert!(probe_failed.provenance().agent().is_none());
}

#[test]
fn workspace_failure_is_explicit_and_the_agent_never_runs() {
    let bundle = snapshot_consented_bundle();
    let judgment = judgment_bytes(&bundle, |_| {});
    let adapter = adapter(
        FakeWorkspace::failing(WorkspaceError::SnapshotUnavailable),
        FakeRunner::returning(judgment),
    );
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderFailure)
    ));
    assert!(snapshot.provenance().run_id().is_none());
    assert_eq!(adapter.runner().calls(), 0);
}
