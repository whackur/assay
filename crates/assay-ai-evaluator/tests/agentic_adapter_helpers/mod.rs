//! Shared helpers for the agentic adapter tests.
#![allow(dead_code)]

use std::{
    cell::RefCell,
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use assay_ai_evaluator::{
    AgentIdentity, AgentRun, AgentRunError, AgentRunner, AgenticConfig, AgenticEvaluator,
    ControlInputs, EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope,
    ExternalTransmission, PreparedWorkspace, ProviderExecutionBoundary, QualitativeRubric,
    SnapshotWorkspace, TransmissionSurface, WorkspaceError,
};
use assay_domain::EvidenceId;
use serde_json::{Value, json};

pub(crate) const ANALYZED_COMMIT: &str =
    "8b7df143d91c716ecfa5fc1730022f6b421b05cedee8fd52b1fc65a96030ad52";

pub(crate) fn evidence_id(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

pub(crate) fn items() -> Vec<EvidenceDescriptor> {
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
pub(crate) fn snapshot_consented_bundle() -> EvidenceBundle {
    EvidenceBundle::with_acknowledged_surface(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        TransmissionSurface::WorktreeSnapshot,
        items(),
    )
    .unwrap()
}

// A public-only bundle whose consent acknowledged only the bundle facts.
pub(crate) fn bundle_only_consent_bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items(),
    )
    .unwrap()
}

pub(crate) fn judgment_bytes(bundle: &EvidenceBundle, mutator: impl FnOnce(&mut Value)) -> Vec<u8> {
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

pub(crate) struct FakeWorkspace {
    pub materialize_error: Option<WorkspaceError>,
    pub seen_commit: RefCell<Option<String>>,
    pub seen_evidence_ids: RefCell<Vec<String>>,
    pub seen_payload: RefCell<Option<String>>,
    pub seen_instructions: RefCell<Option<String>>,
    pub disposed: AtomicUsize,
}

impl FakeWorkspace {
    pub fn ready() -> Self {
        Self {
            materialize_error: None,
            seen_commit: RefCell::new(None),
            seen_evidence_ids: RefCell::new(Vec::new()),
            seen_payload: RefCell::new(None),
            seen_instructions: RefCell::new(None),
            disposed: AtomicUsize::new(0),
        }
    }

    pub fn failing(error: WorkspaceError) -> Self {
        Self {
            materialize_error: Some(error),
            ..Self::ready()
        }
    }

    pub fn disposed(&self) -> usize {
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

pub(crate) struct FakeRunner {
    pub result: Result<Vec<u8>, AgentRunError>,
    pub probe_error: Option<AgentRunError>,
    pub calls: AtomicUsize,
}

impl FakeRunner {
    pub fn returning(judgment: Vec<u8>) -> Self {
        Self {
            result: Ok(judgment),
            probe_error: None,
            calls: AtomicUsize::new(0),
        }
    }

    pub fn failing(error: AgentRunError) -> Self {
        Self {
            result: Err(error),
            probe_error: None,
            calls: AtomicUsize::new(0),
        }
    }

    pub fn unprobeable() -> Self {
        Self {
            result: Err(AgentRunError::Failure),
            probe_error: Some(AgentRunError::ProbeFailed),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn calls(&self) -> usize {
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

pub(crate) fn config() -> AgenticConfig {
    AgenticConfig {
        provider_id: "codex-cli-1",
        model: "gpt-5-codex".to_owned(),
        execution_boundary: ProviderExecutionBoundary::External,
        analyzed_commit: ANALYZED_COMMIT.to_owned(),
    }
}

pub(crate) fn adapter(
    workspace: FakeWorkspace,
    runner: FakeRunner,
) -> AgenticEvaluator<FakeWorkspace, FakeRunner> {
    AgenticEvaluator::new(QualitativeRubric::project_v1(), config(), workspace, runner)
}
