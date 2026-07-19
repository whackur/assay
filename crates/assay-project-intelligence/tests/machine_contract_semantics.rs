use assay_project_intelligence::validate_project_bundle_consistency;

mod machine_contract_helpers;
use machine_contract_helpers::{
    bundle_with_present_feature, feature, feature_mut, feature_related_ids, real_producer_bundle,
    refresh_project_artifact, repository_feature_id, set_feature,
};

#[test]
fn public_contract_validator_closes_feature_state_status_and_citation_semantics() {
    for (status, state, related, analysis_status, expected) in [
        (
            "partial",
            "unavailable",
            &[][..],
            "partial",
            "feature_unavailable_references",
        ),
        (
            "complete",
            "absent",
            &["evidence:tracked-file:v1-golden"][..],
            "complete",
            "feature_absent_references",
        ),
        (
            "partial",
            "present",
            &["evidence:tracked-file:v1-golden"][..],
            "partial",
            "feature_status",
        ),
        (
            "complete",
            "unavailable",
            &["evidence:tracked-file:v1-golden"][..],
            "complete",
            "feature_status",
        ),
        ("partial", "absent", &[][..], "partial", "feature_status"),
    ] {
        let mut bundle = bundle_with_present_feature();
        let id = repository_feature_id(&bundle, "readme", state, related);
        let feature = feature_mut(&mut bundle);
        feature["status"] = serde_json::Value::String(status.into());
        feature["payload"]["state"] = serde_json::Value::String(state.into());
        feature["payload"]["related_evidence_ids"] = serde_json::json!(related);
        feature["id"] = serde_json::Value::String(id);
        bundle["manifest"]["status"] = serde_json::Value::String(analysis_status.into());
        refresh_project_artifact(&mut bundle);
        bundle["manifest"]["artifacts"][0]["status"] =
            serde_json::Value::String(analysis_status.into());

        assert_eq!(
            validate_project_bundle_consistency(&bundle),
            Err(expected),
            "status={status}, state={state}"
        );
    }
}

#[test]
fn public_contract_validator_rejects_path_evidence_for_the_wrong_present_feature() {
    let mut bundle = real_producer_bundle();
    assert_eq!(feature(&bundle, "license")["payload"]["state"], "absent");
    let package_evidence = feature_related_ids(&bundle, "package_manifest");
    set_feature(&mut bundle, "license", "present", &package_evidence);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_semantics")
    );
}

#[test]
fn public_contract_validator_rejects_complete_unrelated_path_evidence_as_an_unavailable_cause() {
    let mut bundle = real_producer_bundle();
    let package_evidence = feature_related_ids(&bundle, "package_manifest");
    set_feature(&mut bundle, "license", "unavailable", &package_evidence);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_semantics")
    );
}

#[test]
fn public_contract_validator_rejects_the_wrong_classification_category_for_a_present_feature() {
    let mut bundle = real_producer_bundle();
    assert_eq!(feature(&bundle, "test")["payload"]["state"], "absent");
    let dependency_evidence = feature_related_ids(&bundle, "dependency");
    set_feature(&mut bundle, "test", "present", &dependency_evidence);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_semantics")
    );
}

#[test]
fn public_contract_validator_rejects_complete_classification_as_an_unavailable_cause() {
    let mut bundle = real_producer_bundle();
    let dependency_evidence = feature_related_ids(&bundle, "dependency");
    set_feature(
        &mut bundle,
        "generated_content",
        "unavailable",
        &dependency_evidence,
    );

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_semantics")
    );
}
