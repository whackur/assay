//! Shared helpers for the evaluator contract tests.
#![allow(dead_code)]

use std::str::FromStr;

use assay_ai_evaluator::{
    EvaluationErrorKind, EvaluationProvider, Evaluator, EvidenceBundle, EvidenceDescriptor,
    EvidenceKind, EvidenceScope, ExternalTransmission, ProviderError, ProviderExecutionBoundary,
    ProviderRequest, QualitativeRubric, TransmissionSurface,
};
use assay_domain::EvidenceId;
use serde_json::{Value, json};

pub(crate) fn evidence_id(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

pub(crate) fn bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::NotUsed,
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
        ],
    )
    .unwrap()
}

pub(crate) fn evaluator() -> Evaluator {
    Evaluator::new(QualitativeRubric::project_v1())
}

pub(crate) fn response(mutator: impl FnOnce(&mut Value)) -> Vec<u8> {
    let bundle = bundle();
    let mut value = json!({
        "schema_version": "1.0.0",
        "evaluation_version": "project-intelligence-1",
        "rubric_version": "project-rubric-1",
        "status": "partial",
        "evidence_bundle_hash": bundle.content_hash(),
        "privacy": {
            "evidence_scope": "public_only",
            "external_transmission": "not_used"
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

pub(crate) fn rejects(response: Vec<u8>, expected: EvaluationErrorKind) {
    use assay_ai_evaluator::DeterministicFakeProvider;
    let provider = DeterministicFakeProvider::from_raw_response(response);
    let error = evaluator().evaluate(&provider, &bundle()).unwrap_err();
    assert_eq!(error.kind(), expected);
    assert!(!format!("{error:?}").contains("The README"));
}

pub(crate) struct FailingProvider;

impl EvaluationProvider for FailingProvider {
    fn provider_id(&self) -> &'static str {
        "failing-test-provider"
    }

    fn execution_boundary(&self) -> ProviderExecutionBoundary {
        ProviderExecutionBoundary::Local
    }

    fn transmission_surface(&self) -> TransmissionSurface {
        TransmissionSurface::BundleOnly
    }

    fn evaluate(&self, _request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError> {
        Err(ProviderError)
    }
}

pub(crate) struct ExternalEchoProvider;

impl EvaluationProvider for ExternalEchoProvider {
    fn provider_id(&self) -> &'static str {
        "external-echo-test-provider"
    }

    fn execution_boundary(&self) -> ProviderExecutionBoundary {
        ProviderExecutionBoundary::External
    }

    fn transmission_surface(&self) -> TransmissionSurface {
        TransmissionSurface::BundleOnly
    }

    fn evaluate(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError> {
        use assay_ai_evaluator::DeterministicFakeProvider;
        DeterministicFakeProvider::valid().evaluate(request)
    }
}
