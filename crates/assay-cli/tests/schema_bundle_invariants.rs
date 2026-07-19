mod schema_contracts_helpers;
use assay_project_intelligence::validate_project_bundle_consistency;
use serde_json::Value;

use schema_contracts_helpers::common::{golden, refresh_project_artifact};
use schema_contracts_helpers::fixtures::{
    classification_record, repository_feature_record, tracked_file_record,
};

#[test]
fn project_analysis_bundle_cross_component_invariants_are_enforced() {
    let bundle = golden("project-analysis");
    validate_project_bundle_consistency(&bundle).expect("reviewed bundle must be coherent");

    let mut foreign_source = bundle.clone();
    foreign_source["evidence"][0]["repository"]["repository_id"] =
        Value::String(format!("sha256:{}", "9".repeat(64)));
    assert!(validate_project_bundle_consistency(&foreign_source).is_err());

    let mut wrong_revision = bundle.clone();
    wrong_revision["evidence"][0]["provenance"]["repository_revision"] =
        Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
    assert!(validate_project_bundle_consistency(&wrong_revision).is_err());

    let mut wrong_count = bundle.clone();
    wrong_count["manifest"]["artifacts"][0]["record_count"] = Value::from(3);
    assert!(validate_project_bundle_consistency(&wrong_count).is_err());

    let mut wrong_role = bundle.clone();
    wrong_role["manifest"]["artifacts"][0]["role"] = Value::String("other".into());
    assert!(validate_project_bundle_consistency(&wrong_role).is_err());

    let mut wrong_status = bundle.clone();
    wrong_status["manifest"]["artifacts"][0]["status"] = Value::String("partial".into());
    assert!(validate_project_bundle_consistency(&wrong_status).is_err());

    let mut wrong_source_revision = bundle.clone();
    wrong_source_revision["manifest"]["data_sources"][0]["revision"] =
        Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
    assert!(validate_project_bundle_consistency(&wrong_source_revision).is_err());

    let mut duplicate = bundle;
    let repeated = duplicate["evidence"][0].clone();
    duplicate["evidence"].as_array_mut().unwrap().push(repeated);
    assert!(validate_project_bundle_consistency(&duplicate).is_err());
}

#[test]
fn project_analysis_bundle_references_are_closed_over_the_evidence_set() {
    let bundle = golden("project-analysis");

    for (case, mutate) in [
        (
            "data source",
            (|value: &mut Value| {
                value["manifest"]["data_sources"][0]["id"] =
                    Value::String("evidence:missing:data-source".into());
            }) as fn(&mut Value),
        ),
        ("warning", |value: &mut Value| {
            value["manifest"]["warnings"] = serde_json::json!([{
                "code": "test_warning",
                "affected_evidence_ids": ["evidence:missing:warning"]
            }]);
        }),
        ("limitation", |value: &mut Value| {
            value["manifest"]["limitations"][0]["affected_evidence_ids"][0] =
                Value::String("evidence:missing:limitation".into());
        }),
        ("top-level relation", |value: &mut Value| {
            value["evidence"][0]["related_evidence_ids"] =
                serde_json::json!(["evidence:missing:top-level"]);
            refresh_project_artifact(value);
        }),
    ] {
        let mut invalid = bundle.clone();
        mutate(&mut invalid);
        assert!(
            validate_project_bundle_consistency(&invalid).is_err(),
            "unknown {case} reference must be rejected"
        );
    }

    let mut payload_relation = bundle;
    let mut feature = tracked_file_record();
    feature["id"] = Value::String("evidence:repository-feature:v1-golden".into());
    feature["provenance"]["content_hash"] = Value::Null;
    feature["payload"] = serde_json::json!({
        "kind": "repository_feature",
        "feature": "readme",
        "state": "present",
        "related_evidence_ids": ["evidence:missing:payload"]
    });
    payload_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .push(feature);
    refresh_project_artifact(&mut payload_relation);
    assert!(
        validate_project_bundle_consistency(&payload_relation).is_err(),
        "unknown payload relation must be rejected"
    );
}

#[test]
fn project_analysis_bundle_classification_citations_and_policy_are_consistent() {
    let mut bundle = golden("project-analysis");
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .extend([tracked_file_record(), classification_record()]);
    refresh_project_artifact(&mut bundle);
    validate_project_bundle_consistency(&bundle).expect("coherent classification must validate");

    let mut wrong_relation = bundle.clone();
    let classification = wrong_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["related_evidence_ids"] =
        serde_json::json!(["evidence:history-scope:v1-golden"]);
    refresh_project_artifact(&mut wrong_relation);
    assert!(validate_project_bundle_consistency(&wrong_relation).is_err());

    let mut extra_relation = bundle.clone();
    let classification = extra_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["related_evidence_ids"] = serde_json::json!([
        "evidence:history-scope:v1-golden",
        "evidence:tracked-file:v1-golden"
    ]);
    refresh_project_artifact(&mut extra_relation);
    assert!(validate_project_bundle_consistency(&extra_relation).is_err());

    let mut wrong_policy = bundle;
    let classification = wrong_policy["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["attempted_policy_version"] = Value::String("classifier-v2".into());
    refresh_project_artifact(&mut wrong_policy);
    assert!(validate_project_bundle_consistency(&wrong_policy).is_err());
}

#[test]
fn project_analysis_bundle_snapshot_and_history_facts_match_the_manifest() {
    let bundle = golden("project-analysis");
    for (case, mutate) in [
        (
            "snapshot root tree",
            (|value: &mut Value| {
                let snapshot = value["evidence"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|fact| fact["payload"]["kind"] == "repository_snapshot")
                    .unwrap();
                snapshot["payload"]["root_tree"] =
                    Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
                refresh_project_artifact(value);
            }) as fn(&mut Value),
        ),
        ("snapshot commit time", |value: &mut Value| {
            let snapshot = value["evidence"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|fact| fact["payload"]["kind"] == "repository_snapshot")
                .unwrap();
            snapshot["payload"]["commit_time"] = Value::String("2026-01-02T03:04:04Z".into());
            refresh_project_artifact(value);
        }),
        ("history count", |value: &mut Value| {
            value["manifest"]["scope"]["commit_count"] = Value::from(4);
        }),
        ("history head", |value: &mut Value| {
            value["manifest"]["scope"]["head_revision"] =
                Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
        }),
        ("history status", |value: &mut Value| {
            value["manifest"]["scope"]["history_status"] = Value::String("partial".into());
        }),
        ("repository data-source status", |value: &mut Value| {
            value["manifest"]["data_sources"][0]["status"] = Value::String("partial".into());
        }),
        ("history data-source status", |value: &mut Value| {
            value["manifest"]["data_sources"][1]["status"] = Value::String("partial".into());
        }),
    ] {
        let mut invalid = bundle.clone();
        mutate(&mut invalid);
        assert!(
            validate_project_bundle_consistency(&invalid).is_err(),
            "mismatched {case} must be rejected"
        );
    }
}

#[test]
fn project_analysis_bundle_content_hash_and_status_redundancy_is_closed() {
    let mut tracked = golden("project-analysis");
    tracked["evidence"]
        .as_array_mut()
        .unwrap()
        .push(tracked_file_record());
    refresh_project_artifact(&mut tracked);
    validate_project_bundle_consistency(&tracked).expect("coherent tracked file must validate");

    let mut wrong_hash = tracked;
    let fact = wrong_hash["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "tracked_file")
        .unwrap();
    fact["provenance"]["content_hash"] = Value::String(format!("sha256:{}", "5".repeat(64)));
    refresh_project_artifact(&mut wrong_hash);
    assert!(validate_project_bundle_consistency(&wrong_hash).is_err());

    let mut complete_with_partial_evidence = golden("project-analysis");
    complete_with_partial_evidence["evidence"][0]["status"] = Value::String("partial".into());
    refresh_project_artifact(&mut complete_with_partial_evidence);
    assert!(validate_project_bundle_consistency(&complete_with_partial_evidence).is_err());

    let mut unjustified_partial = golden("project-analysis");
    unjustified_partial["manifest"]["status"] = Value::String("partial".into());
    unjustified_partial["manifest"]["artifacts"][0]["status"] = Value::String("partial".into());
    assert!(validate_project_bundle_consistency(&unjustified_partial).is_err());
}

#[test]
fn repository_feature_schema_closes_state_status_and_citation_cardinality() {
    let validator = schema_contracts_helpers::common::validator("project-evidence");
    for (case, status, state, related) in [
        (
            "present without supporting evidence",
            "complete",
            "present",
            serde_json::json!([]),
        ),
        (
            "unavailable without cause evidence",
            "partial",
            "unavailable",
            serde_json::json!([]),
        ),
        (
            "absent with contradictory evidence",
            "complete",
            "absent",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "partial present feature",
            "partial",
            "present",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "complete unavailable feature",
            "complete",
            "unavailable",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
    ] {
        schema_contracts_helpers::common::assert_rejected(
            &validator,
            &repository_feature_record(status, state, related),
            "project-evidence",
            case,
        );
    }

    for (case, status, state, related) in [
        (
            "cited present feature",
            "complete",
            "present",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "cited unavailable feature",
            "partial",
            "unavailable",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "uncited absent feature",
            "complete",
            "absent",
            serde_json::json!([]),
        ),
    ] {
        let feature = repository_feature_record(status, state, related);
        let errors = schema_contracts_helpers::common::validation_messages(&validator, &feature);
        assert!(
            errors.is_empty(),
            "{case} failed project-evidence: {errors:#?}"
        );
    }
}
