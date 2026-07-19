//! Agentic adapter success, provenance, and control-input tests.

mod agentic_adapter_helpers;

use assay_ai_evaluator::{
    AgenticConfig, AgenticEvaluator, EvidenceBundle, EvidenceScope, ExternalTransmission,
    ProviderExecutionBoundary, QualitativeRubric, SnapshotOutcome, TransmissionSurface,
};
use serde_json::Value;

use agentic_adapter_helpers::{
    ANALYZED_COMMIT, FakeRunner, FakeWorkspace, adapter, bundle_only_consent_bundle, config, items,
    judgment_bytes, snapshot_consented_bundle,
};

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
    use assay_ai_evaluator::EvaluationErrorKind;
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
    use serde_json::json;
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
