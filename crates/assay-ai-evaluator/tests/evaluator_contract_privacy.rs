//! Privacy, transmission surface, and provider boundary tests.

mod evaluator_contract_helpers;

use assay_ai_evaluator::{
    EvaluationErrorKind, EvaluationProvider, EvidenceBundle, EvidenceDescriptor, EvidenceKind,
    EvidenceScope, ExternalTransmission, ProviderError, ProviderExecutionBoundary, ProviderRequest,
    TransmissionSurface,
};
use serde_json::Value;

use evaluator_contract_helpers::{ExternalEchoProvider, bundle, evaluator, evidence_id};

#[test]
fn private_evidence_requires_explicit_transmission_policy() {
    let item = EvidenceDescriptor::new(
        evidence_id("evidence:repository:snapshot"),
        EvidenceKind::RepositoryFact,
        "The source revision is immutable.",
    )
    .unwrap();
    let error = EvidenceBundle::new(
        EvidenceScope::PrivateLocal,
        ExternalTransmission::PublicOnly,
        vec![item],
    )
    .unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::PrivacyMismatch);
}

#[test]
fn provider_request_separates_fixed_instructions_from_delimited_evidence() {
    use assay_ai_evaluator::DeterministicFakeProvider;
    struct InspectingProvider;

    impl EvaluationProvider for InspectingProvider {
        fn provider_id(&self) -> &'static str {
            "inspecting-test-provider"
        }

        fn execution_boundary(&self) -> ProviderExecutionBoundary {
            ProviderExecutionBoundary::Local
        }

        fn transmission_surface(&self) -> TransmissionSurface {
            TransmissionSurface::BundleOnly
        }

        fn evaluate(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError> {
            assert!(!request.system_instructions().contains("README describes"));
            let payload: Value = serde_json::from_str(request.canonical_payload()).unwrap();
            assert_eq!(payload["repository_evidence_is_untrusted_data"], true);
            assert_eq!(payload["end_evidence"], true);
            assert_eq!(payload["privacy"]["external_transmission"], "not_used");
            assert_eq!(payload["begin_evidence"].as_array().unwrap().len(), 2);
            DeterministicFakeProvider::valid().evaluate(request)
        }
    }

    evaluator()
        .evaluate(&InspectingProvider, &bundle())
        .unwrap();
}

#[test]
fn local_provider_cannot_cross_an_external_transmission_boundary() {
    use assay_ai_evaluator::DeterministicFakeProvider;
    let bundle = EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:repository:snapshot"),
                EvidenceKind::RepositoryFact,
                "The source revision is immutable.",
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let error = evaluator()
        .evaluate(&DeterministicFakeProvider::valid(), &bundle)
        .unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::PrivacyMismatch);
}

#[test]
fn external_provider_with_consented_private_evidence_is_accepted() {
    let bundle = EvidenceBundle::new(
        EvidenceScope::PrivateLocal,
        ExternalTransmission::ConsentedPrivate,
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:repository:snapshot"),
                EvidenceKind::RepositoryFact,
                "The source revision is immutable.",
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let result = evaluator()
        .evaluate(&ExternalEchoProvider, &bundle)
        .unwrap();
    assert_eq!(result.judgments().len(), 4);
    assert_eq!(result.evidence_bundle_hash(), bundle.content_hash());
}

#[test]
fn external_provider_cannot_read_private_local_evidence_without_consent() {
    let bundle = EvidenceBundle::new(
        EvidenceScope::PrivateLocal,
        ExternalTransmission::NotUsed,
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:repository:snapshot"),
                EvidenceKind::RepositoryFact,
                "The source revision is immutable.",
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let error = evaluator()
        .evaluate(&ExternalEchoProvider, &bundle)
        .unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::PrivacyMismatch);
}

#[test]
fn snapshot_surface_provider_requires_snapshot_consent_even_for_public_repos() {
    use assay_ai_evaluator::DeterministicFakeProvider;
    struct SnapshotSurfaceProvider;

    impl EvaluationProvider for SnapshotSurfaceProvider {
        fn provider_id(&self) -> &'static str {
            "snapshot-surface-test-provider"
        }

        fn execution_boundary(&self) -> ProviderExecutionBoundary {
            ProviderExecutionBoundary::External
        }

        fn transmission_surface(&self) -> TransmissionSurface {
            TransmissionSurface::WorktreeSnapshot
        }

        fn evaluate(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError> {
            DeterministicFakeProvider::valid().evaluate(request)
        }
    }

    let items = || {
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:repository:snapshot"),
                EvidenceKind::RepositoryFact,
                "The source revision is immutable.",
            )
            .unwrap(),
        ]
    };

    // Public-only consent that acknowledged only the bundle facts is not
    // sufficient for a provider that can transmit the whole snapshot.
    let bundle_only = EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items(),
    )
    .unwrap();
    let error = evaluator()
        .evaluate(&SnapshotSurfaceProvider, &bundle_only)
        .unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::PrivacyMismatch);

    // Consent that acknowledged the worktree-snapshot surface by name passes.
    let snapshot_acknowledged = EvidenceBundle::with_acknowledged_surface(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        TransmissionSurface::WorktreeSnapshot,
        items(),
    )
    .unwrap();
    let result = evaluator()
        .evaluate(&SnapshotSurfaceProvider, &snapshot_acknowledged)
        .unwrap();
    assert_eq!(result.judgments().len(), 4);
}

#[test]
fn acknowledged_surface_does_not_change_the_bundle_content_hash() {
    let items = || {
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:repository:snapshot"),
                EvidenceKind::RepositoryFact,
                "The source revision is immutable.",
            )
            .unwrap(),
        ]
    };
    let bundle_only = EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items(),
    )
    .unwrap();
    let snapshot_acknowledged = EvidenceBundle::with_acknowledged_surface(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        TransmissionSurface::WorktreeSnapshot,
        items(),
    )
    .unwrap();
    // The surface gates transmission; judgments bind to evidence content.
    assert_eq!(
        bundle_only.content_hash(),
        snapshot_acknowledged.content_hash()
    );
}
