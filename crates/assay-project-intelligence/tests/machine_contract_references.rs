use assay_project_intelligence::validate_project_bundle_consistency;
use serde_json::Value;

mod machine_contract_helpers;
use machine_contract_helpers::{
    bundle_with_present_feature, coherent_bundle, feature_mut, real_producer_bundle,
    refresh_project_artifact, repository_feature_id, tracked_file_record,
};

#[test]
fn public_contract_validator_accepts_the_reviewed_coherent_bundle() {
    validate_project_bundle_consistency(&coherent_bundle())
        .expect("reviewed project-analysis golden must be coherent");
}

#[test]
fn public_contract_validator_rejects_a_dangling_data_source() {
    let mut bundle = coherent_bundle();
    bundle["manifest"]["data_sources"][0]["id"] =
        Value::String("evidence:missing:data-source".into());
    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("unknown_evidence_reference")
    );
}

#[test]
fn public_contract_validator_rejects_a_dangling_classification_payload_citation() {
    let mut bundle = real_producer_bundle();
    let classification = bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["payload"]["source_evidence_id"] =
        Value::String("evidence:missing:tracked-file".into());
    refresh_project_artifact(&mut bundle);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("unknown_evidence_reference")
    );
}

#[test]
fn public_contract_validator_rejects_a_dangling_diagnostic_citation() {
    let mut bundle = coherent_bundle();
    bundle["manifest"]["limitations"][0]["affected_evidence_ids"][0] =
        Value::String("evidence:missing:diagnostic-target".into());

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("unknown_evidence_reference")
    );
}

#[test]
fn public_contract_validator_rejects_a_present_feature_without_citations_even_with_a_new_id() {
    let mut bundle = bundle_with_present_feature();
    let id = repository_feature_id(&bundle, "readme", "present", &[]);
    let feature = feature_mut(&mut bundle);
    feature["payload"]["related_evidence_ids"] = serde_json::json!([]);
    feature["id"] = Value::String(id);
    refresh_project_artifact(&mut bundle);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_present_references")
    );
}

#[test]
fn public_contract_validator_binds_feature_identity_to_ordered_citations() {
    let mut bundle = bundle_with_present_feature();
    let mut second = tracked_file_record();
    second["id"] = Value::String("evidence:tracked-file:v1-second".into());
    bundle["evidence"].as_array_mut().unwrap().push(second);
    feature_mut(&mut bundle)["payload"]["related_evidence_ids"] = serde_json::json!([
        "evidence:tracked-file:v1-golden",
        "evidence:tracked-file:v1-second"
    ]);
    refresh_project_artifact(&mut bundle);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_identity")
    );
}

#[test]
fn public_contract_validator_rejects_noncanonical_feature_citation_order() {
    let mut bundle = bundle_with_present_feature();
    let mut second = tracked_file_record();
    second["id"] = Value::String("evidence:tracked-file:v1-second".into());
    bundle["evidence"].as_array_mut().unwrap().push(second);
    let related = [
        "evidence:tracked-file:v1-second",
        "evidence:tracked-file:v1-golden",
    ];
    let id = repository_feature_id(&bundle, "readme", "present", &related);
    let feature = feature_mut(&mut bundle);
    feature["payload"]["related_evidence_ids"] = serde_json::json!(related);
    feature["id"] = Value::String(id);
    refresh_project_artifact(&mut bundle);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_reference_order")
    );
}
