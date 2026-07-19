//! Agentic adapter failure, validation, and boundary tests.

mod agentic_adapter_helpers;

use assay_ai_evaluator::{AgentRunError, EvaluationErrorKind, SnapshotOutcome, WorkspaceError};
use serde_json::json;

use agentic_adapter_helpers::{
    FakeRunner, FakeWorkspace, adapter, judgment_bytes, snapshot_consented_bundle,
};

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
