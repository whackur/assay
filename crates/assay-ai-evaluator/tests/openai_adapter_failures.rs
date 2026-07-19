//! OpenAI adapter failure, privacy, and secret-handling tests.

mod openai_adapter_helpers;

use std::time::Duration;

use assay_ai_evaluator::{
    EvaluationErrorKind, EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope,
    ExternalTransmission, SnapshotOutcome, TransportError,
};
use serde_json::json;

use openai_adapter_helpers::FakeStore;
use openai_adapter_helpers::{
    SECRET_VALUE, ScriptedTransport, bundle, chat_envelope, evaluator, evidence_id, judgment_body,
    usage,
};

#[test]
fn schema_invalid_provider_output_is_explicit_not_success() {
    let mut invalid: serde_json::Value = serde_json::from_str(&judgment_body()).unwrap();
    invalid["judgments"][0]["rating"] = json!(9);
    let transport = ScriptedTransport::ok(
        chat_envelope(&serde_json::to_string(&invalid).unwrap(), usage(10, 5)),
        Duration::from_millis(40),
    );
    let adapter = evaluator(FakeStore::with_key(SECRET_VALUE), transport);
    let snapshot = adapter.evaluate(&bundle());

    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::InvalidRating)
    ));
    assert!(snapshot.validated().is_none());
    let telemetry = snapshot
        .telemetry()
        .expect("telemetry retained on validation failure");
    assert_eq!(telemetry.http_status(), 200);
}

#[test]
fn malformed_envelope_is_reported_as_malformed_output() {
    let transport = ScriptedTransport::ok(b"not-json".to_vec(), Duration::from_millis(3));
    let adapter = evaluator(FakeStore::with_key(SECRET_VALUE), transport);
    let snapshot = adapter.evaluate(&bundle());
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::MalformedOutput)
    ));
}

#[test]
fn timeout_is_an_explicit_status() {
    let adapter = evaluator(
        FakeStore::with_key(SECRET_VALUE),
        ScriptedTransport::failing(TransportError::Timeout),
    );
    let snapshot = adapter.evaluate(&bundle());
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderTimeout)
    ));
    assert_eq!(snapshot.outcome().status_code(), "provider_timeout");
    assert!(snapshot.telemetry().is_none());
}

#[test]
fn rate_limit_is_an_explicit_status() {
    let adapter = evaluator(
        FakeStore::with_key(SECRET_VALUE),
        ScriptedTransport::status(429),
    );
    let snapshot = adapter.evaluate(&bundle());
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderRateLimited)
    ));
    let telemetry = snapshot
        .telemetry()
        .expect("telemetry from an http response");
    assert_eq!(telemetry.http_status(), 429);
}

#[test]
fn unauthorized_status_supports_key_rotation_signalling() {
    let adapter = evaluator(
        FakeStore::with_key(SECRET_VALUE),
        ScriptedTransport::status(401),
    );
    let snapshot = adapter.evaluate(&bundle());
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::ProviderUnauthorized)
    ));
}

#[test]
fn missing_secret_fails_closed_without_calling_the_transport() {
    let transport = ScriptedTransport::ok(
        chat_envelope(&judgment_body(), usage(1, 1)),
        Duration::from_millis(1),
    );
    let adapter = evaluator(FakeStore::unavailable(), transport);
    let snapshot = adapter.evaluate(&bundle());
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::SecretUnavailable)
    ));
    assert_eq!(adapter.transport().calls(), 0);
}

#[test]
fn private_local_evidence_without_consent_never_reaches_the_transport() {
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
    let transport = ScriptedTransport::ok(
        chat_envelope(&judgment_body(), usage(1, 1)),
        Duration::from_millis(1),
    );
    let adapter = evaluator(FakeStore::with_key(SECRET_VALUE), transport);
    let snapshot = adapter.evaluate(&bundle);
    assert!(matches!(
        snapshot.outcome(),
        SnapshotOutcome::Failed(EvaluationErrorKind::PrivacyMismatch)
    ));
    assert_eq!(adapter.transport().calls(), 0);
}
