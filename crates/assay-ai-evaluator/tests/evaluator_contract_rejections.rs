//! Provider output rejection tests for malformed judgments.

mod evaluator_contract_helpers;

use assay_ai_evaluator::EvaluationErrorKind;
use serde_json::{Value, json};

use evaluator_contract_helpers::{rejects, response};

#[test]
fn rejects_unknown_criterion() {
    rejects(
        response(|value| value["judgments"][0]["criterion_id"] = json!("person.productivity")),
        EvaluationErrorKind::UnknownCriterion,
    );
}

#[test]
fn rejects_out_of_range_rating() {
    rejects(
        response(|value| value["judgments"][0]["rating"] = json!(5)),
        EvaluationErrorKind::InvalidRating,
    );
}

#[test]
fn rejects_missing_citation() {
    rejects(
        response(|value| value["judgments"][0]["evidence_ids"] = json!([])),
        EvaluationErrorKind::MissingCitation,
    );
}

#[test]
fn rejects_forged_evidence_id() {
    rejects(
        response(|value| value["judgments"][0]["evidence_ids"] = json!(["evidence:source:forged"])),
        EvaluationErrorKind::UnknownEvidenceCitation,
    );
}

#[test]
fn rejects_schema_invalid_provider_output() {
    rejects(
        response(|value| value["judgments"][0]["unexpected"] = json!(true)),
        EvaluationErrorKind::SchemaInvalid,
    );
    rejects(b"not-json".to_vec(), EvaluationErrorKind::MalformedOutput);
}

#[test]
fn rejects_bundle_hash_and_privacy_forgery() {
    rejects(
        response(|value| {
            value["evidence_bundle_hash"] = json!(format!("sha256:{}", "f".repeat(64)))
        }),
        EvaluationErrorKind::EvidenceBundleMismatch,
    );
    rejects(
        response(|value| value["privacy"]["external_transmission"] = json!("public_only")),
        EvaluationErrorKind::PrivacyMismatch,
    );
}

#[test]
fn rejects_duplicate_criteria_and_citations() {
    rejects(
        response(|value| {
            let duplicate = value["judgments"][0].clone();
            value["judgments"].as_array_mut().unwrap().push(duplicate);
        }),
        EvaluationErrorKind::DuplicateCriterion,
    );
    rejects(
        response(|value| {
            value["judgments"][0]["evidence_ids"] =
                json!(["evidence:readme:claim-4", "evidence:readme:claim-4"])
        }),
        EvaluationErrorKind::DuplicateCitation,
    );
}

#[test]
fn complete_output_must_cover_the_whole_versioned_rubric() {
    rejects(
        response(|value| value["status"] = json!("complete")),
        EvaluationErrorKind::MissingCriterion,
    );
}

#[test]
fn rejects_version_and_hash_binding_mismatches() {
    rejects(
        response(|value| value["schema_version"] = json!("2.0.0")),
        EvaluationErrorKind::SchemaInvalid,
    );
    rejects(
        response(|value| value["evaluation_version"] = json!("project-intelligence-2")),
        EvaluationErrorKind::EvaluationVersionMismatch,
    );
    rejects(
        response(|value| value["rubric_version"] = json!("project-rubric-2")),
        EvaluationErrorKind::RubricVersionMismatch,
    );
    rejects(
        response(|value| value["evidence_bundle_hash"] = json!("sha256:not-a-real-digest")),
        EvaluationErrorKind::SchemaInvalid,
    );
}

#[test]
fn rejects_invalid_confidence_and_rating_shapes() {
    rejects(
        response(|value| value["judgments"][0]["confidence"] = json!(1.5)),
        EvaluationErrorKind::InvalidConfidence,
    );
    rejects(
        response(|value| value["judgments"][0]["rating_scale"] = json!(3)),
        EvaluationErrorKind::InvalidRating,
    );
    rejects(
        response(|value| value["judgments"][0]["rating"] = Value::Null),
        EvaluationErrorKind::InvalidRating,
    );
    rejects(
        response(|value| value["judgments"][0]["applicability"] = json!("not_applicable")),
        EvaluationErrorKind::InvalidRating,
    );
}

#[test]
fn rejects_status_and_judgment_inconsistency() {
    rejects(
        response(|value| value["status"] = json!("unavailable")),
        EvaluationErrorKind::SchemaInvalid,
    );
    rejects(
        response(|value| value["judgments"] = json!([])),
        EvaluationErrorKind::SchemaInvalid,
    );
}

#[test]
fn rejects_output_larger_than_the_size_limit() {
    use assay_ai_evaluator::DeterministicFakeProvider;
    use evaluator_contract_helpers::{bundle, evaluator};
    let provider = DeterministicFakeProvider::from_raw_response(vec![b' '; 64 * 1024 + 1]);
    let error = evaluator().evaluate(&provider, &bundle()).unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::OutputTooLarge);
}

#[test]
fn provider_failure_is_redacted_to_a_stable_category() {
    use assay_ai_evaluator::ProviderError;
    use evaluator_contract_helpers::{FailingProvider, bundle, evaluator};
    let error = evaluator()
        .evaluate(&FailingProvider, &bundle())
        .unwrap_err();
    assert_eq!(error.kind(), EvaluationErrorKind::ProviderFailure);
    assert_eq!(format!("{error}"), "provider_failure");
    assert_eq!(format!("{}", ProviderError), "provider_failure");
}
