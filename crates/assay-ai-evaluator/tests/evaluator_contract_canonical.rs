//! Deterministic fake provider canonical judgment and schema validation tests.

mod evaluator_contract_helpers;

use assay_ai_evaluator::DeterministicFakeProvider;
use serde_json::Value;

use evaluator_contract_helpers::{bundle, evaluator};

#[test]
fn deterministic_fake_provider_returns_canonical_validated_judgments() {
    let evaluator = evaluator();
    let provider = DeterministicFakeProvider::valid();
    let first = evaluator.evaluate(&provider, &bundle()).unwrap();
    let second = evaluator.evaluate(&provider, &bundle()).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.rubric_version(), "project-rubric-1");
    assert_eq!(first.evidence_bundle_hash(), bundle().content_hash());
    assert_eq!(first.judgments().len(), 4);
    assert!(
        first
            .judgments()
            .windows(2)
            .all(|pair| pair[0].criterion_id() < pair[1].criterion_id())
    );
    assert!(first.judgments().iter().all(|judgment| {
        judgment
            .evidence_ids()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    }));

    let serialized = serde_json::to_value(&first).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/ai-judgment/v1.json")).unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&serialized));
    assert!(serialized.get("assay_score").is_none());
    assert!(serialized.get("subjects").is_none());
}

#[test]
fn score_compiler_view_excludes_provider_prose() {
    let result = evaluator()
        .evaluate(&DeterministicFakeProvider::valid(), &bundle())
        .unwrap();
    let scoring = result.scoring_judgments().collect::<Vec<_>>();

    assert_eq!(scoring.len(), 4);
    assert!(scoring.iter().all(|judgment| judgment.rating().is_some()));
    assert!(format!("{scoring:?}").find("rationale").is_none());
    assert!(!format!("{result:?}").contains("bounded project criterion"));
}

#[test]
fn accepts_not_applicable_judgment_without_citations() {
    let response = evaluator_contract_helpers::response(|value| {
        value["judgments"][0]["applicability"] = serde_json::json!("not_applicable");
        value["judgments"][0]["rating"] = Value::Null;
        value["judgments"][0]["evidence_ids"] = serde_json::json!([]);
    });
    let provider = DeterministicFakeProvider::from_raw_response(response);
    let result = evaluator().evaluate(&provider, &bundle()).unwrap();

    assert_eq!(result.judgments().len(), 1);
    assert!(result.judgments()[0].rating().is_none());
    assert!(result.judgments()[0].evidence_ids().is_empty());

    let serialized = serde_json::to_value(&result).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/ai-judgment/v1.json")).unwrap();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&serialized));
}

#[test]
fn bundle_hash_is_canonical_across_input_order() {
    use assay_ai_evaluator::{EvidenceBundle, EvidenceScope, ExternalTransmission};
    let first = bundle();
    let reversed = EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::NotUsed,
        first.items().iter().cloned().rev().collect(),
    )
    .unwrap();

    assert_eq!(first, reversed);
    assert_eq!(first.content_hash(), reversed.content_hash());
}

#[test]
fn validated_judgments_map_onto_the_shared_domain_compiler_contract() {
    let bundle = bundle();
    let validated = evaluator()
        .evaluate(&DeterministicFakeProvider::valid(), &bundle)
        .expect("the deterministic provider must produce a valid judgment set");
    let domain_set = validated
        .to_rubric_judgment_set()
        .expect("validated judgments must map onto the domain contract");

    assert_eq!(
        domain_set.evaluation_version().as_str(),
        "project-intelligence-1"
    );
    assert_eq!(domain_set.rubric_version().as_str(), "project-rubric-1");
    assert_eq!(
        domain_set.evidence_bundle_hash().as_str(),
        bundle.content_hash()
    );
    assert_eq!(domain_set.judgments().len(), validated.judgments().len());
    // Every mapped citation stays within the bundle the provider was shown.
    for judgment in domain_set.judgments() {
        for evidence in judgment.evidence_ids() {
            assert!(
                bundle.items().iter().any(|item| item.id() == evidence),
                "a mapped citation must remain inside the evidence bundle"
            );
        }
    }
}
