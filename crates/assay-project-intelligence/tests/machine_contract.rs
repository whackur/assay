use assay_project_intelligence::validate_project_bundle_consistency;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn coherent_bundle() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/golden/project-analysis-v1.json"
    ))
    .expect("reviewed project-analysis golden must parse")
}

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
    feature_mut(&mut bundle)["payload"]["related_evidence_ids"] =
        serde_json::json!(["evidence:tracked-file:v1-second"]);
    refresh_project_artifact(&mut bundle);

    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("feature_identity")
    );
}

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
        feature["status"] = Value::String(status.into());
        feature["payload"]["state"] = Value::String(state.into());
        feature["payload"]["related_evidence_ids"] = serde_json::json!(related);
        feature["id"] = Value::String(id);
        bundle["manifest"]["status"] = Value::String(analysis_status.into());
        refresh_project_artifact(&mut bundle);
        bundle["manifest"]["artifacts"][0]["status"] = Value::String(analysis_status.into());

        assert_eq!(
            validate_project_bundle_consistency(&bundle),
            Err(expected),
            "status={status}, state={state}"
        );
    }
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

fn bundle_with_present_feature() -> Value {
    let mut bundle = coherent_bundle();
    let tracked = tracked_file_record();
    let related = [tracked["id"].as_str().unwrap()];
    let id = repository_feature_id(&bundle, "readme", "present", &related);
    let feature = serde_json::json!({
        "schema_version": "1.0.0",
        "repository": bundle["manifest"]["source_snapshot"]["source"].clone(),
        "id": id,
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": bundle["manifest"]["source_snapshot"]["revision"].clone(),
            "content_hash": null,
            "remote_record_id": null
        },
        "payload": {
            "kind": "repository_feature",
            "feature": "readme",
            "state": "present",
            "related_evidence_ids": related
        }
    });
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .extend([tracked, feature]);
    refresh_project_artifact(&mut bundle);
    validate_project_bundle_consistency(&bundle).expect("baseline feature bundle must validate");
    bundle
}

fn tracked_file_record() -> Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "repository": {
            "kind": "local",
            "repository_id": format!("sha256:{}", "1".repeat(64))
        },
        "id": "evidence:tracked-file:v1-golden",
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": "0123456789abcdef0123456789abcdef01234567",
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "remote_record_id": null
        },
        "payload": {
            "kind": "tracked_file",
            "path": { "encoding": "utf8", "value": "README.md" },
            "mode": "regular",
            "object_kind": "blob",
            "object_id": "89abcdef0123456789abcdef0123456789abcdef",
            "content_status": "complete",
            "language": "Markdown",
            "language_status": "complete",
            "size_bytes": 12,
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "issue": null
        }
    })
}

fn feature_mut(bundle: &mut Value) -> &mut Value {
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "repository_feature")
        .unwrap()
}

fn repository_feature_id(bundle: &Value, feature: &str, state: &str, related: &[&str]) -> String {
    let source = &bundle["manifest"]["source_snapshot"]["source"];
    let identity_scope = format!("local:{}", source["repository_id"].as_str().unwrap());
    let revision = bundle["manifest"]["source_snapshot"]["revision"]
        .as_str()
        .unwrap();
    let input = format!(
        "{identity_scope}\0{revision}\0{feature}\0{state}\0{}",
        related.join("\0")
    );
    let digest = format!("{:x}", Sha256::digest(input.as_bytes()));
    format!("evidence:repository-feature:v1-{}", &digest[..24])
}

fn refresh_project_artifact(bundle: &mut Value) {
    let evidence = bundle["evidence"].as_array_mut().unwrap();
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let bytes = serde_json::to_vec(evidence).unwrap();
    bundle["manifest"]["artifacts"][0]["record_count"] = Value::from(evidence.len());
    bundle["manifest"]["artifacts"][0]["content_hash"] =
        Value::String(format!("sha256:{:x}", Sha256::digest(bytes)));
}
