mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{assert_rejected, golden, validator};
use schema_contracts_helpers::fixtures::classification_record;

#[test]
fn parent_delta_status_issue_and_observed_counts_are_bidirectional() {
    let validator = validator("project-evidence");
    let mut complete = golden("project-evidence");
    complete["payload"] = serde_json::json!({
        "kind": "parent_delta",
        "changed_entries": 3,
        "renames": 1,
        "issue": null
    });
    assert!(validator.is_valid(&complete));

    let mut partial = complete.clone();
    partial["status"] = Value::String("partial".into());
    partial["payload"]["renames"] = Value::Null;
    partial["payload"]["issue"] = Value::String("rename_candidate_limit".into());
    assert!(validator.is_valid(&partial));

    let mut complete_with_issue = partial.clone();
    complete_with_issue["status"] = Value::String("complete".into());
    assert_rejected(
        &validator,
        &complete_with_issue,
        "project-evidence",
        "complete parent delta with issue",
    );

    let mut partial_without_issue = complete;
    partial_without_issue["status"] = Value::String("partial".into());
    assert_rejected(
        &validator,
        &partial_without_issue,
        "project-evidence",
        "partial parent delta without issue",
    );

    let mut fabricated_rename_count = partial;
    fabricated_rename_count["payload"]["renames"] = Value::from(0);
    assert_rejected(
        &validator,
        &fabricated_rename_count,
        "project-evidence",
        "rename-limit parent delta with fabricated rename count",
    );

    let mut process_failure_payload = golden("project-evidence");
    process_failure_payload["status"] = Value::String("partial".into());
    process_failure_payload["payload"] = serde_json::json!({
        "kind": "parent_delta",
        "changed_entries": null,
        "renames": null,
        "issue": "process_failure"
    });
    assert_rejected(
        &validator,
        &process_failure_payload,
        "project-evidence",
        "process failure represented as a factual parent delta",
    );

    let mut process_failure_envelope = golden("project-evidence");
    process_failure_envelope["status"] = Value::String("unavailable".into());
    process_failure_envelope["grade"] = Value::Null;
    process_failure_envelope
        .as_object_mut()
        .unwrap()
        .remove("provenance");
    process_failure_envelope
        .as_object_mut()
        .unwrap()
        .remove("payload");
    process_failure_envelope["requested_kind"] = Value::String("parent_delta".into());
    process_failure_envelope["reason"] = Value::String("process_failure".into());
    assert!(validator.is_valid(&process_failure_envelope));
}

#[test]
fn classification_availability_evidence_and_envelope_policy_are_consistent() {
    let validator = validator("project-evidence");
    let mut partial = classification_record();
    partial["status"] = Value::String("partial".into());
    partial["payload"]["reason"] = Value::String("attributes_unavailable".into());
    partial["payload"]["classification"]["evidence"] = serde_json::json!([
        {
            "kind": "policy_rule",
            "rule_id": "path.production.typescript",
            "attribute_name": null,
            "attribute_value": null
        },
        {
            "kind": "attribute_facts_unavailable",
            "rule_id": "attributes.unavailable",
            "attribute_name": null,
            "attribute_value": null
        }
    ]);
    assert!(validator.is_valid(&partial));

    let mut partial_without_unavailable_fact = partial.clone();
    partial_without_unavailable_fact["payload"]["classification"]["evidence"] = serde_json::json!([{
        "kind": "policy_rule",
        "rule_id": "path.production.typescript",
        "attribute_name": null,
        "attribute_value": null
    }]);
    assert_rejected(
        &validator,
        &partial_without_unavailable_fact,
        "project-evidence",
        "partial classification without unavailable attribute evidence",
    );

    let mut complete_with_unavailable_fact = partial;
    complete_with_unavailable_fact["status"] = Value::String("complete".into());
    complete_with_unavailable_fact["payload"]["reason"] = Value::Null;
    assert_rejected(
        &validator,
        &complete_with_unavailable_fact,
        "project-evidence",
        "complete classification with unavailable attribute evidence",
    );

    let mut missing = golden("project-evidence");
    missing["status"] = Value::String("unavailable".into());
    missing["grade"] = Value::Null;
    missing.as_object_mut().unwrap().remove("provenance");
    missing.as_object_mut().unwrap().remove("payload");
    missing["requested_kind"] = Value::String("file_classification".into());
    missing["reason"] = Value::String("missing_classification".into());
    missing["related_evidence_ids"] = serde_json::json!(["evidence:tracked-file:v1-golden"]);
    assert!(
        validator.is_valid(&missing),
        "missing classification must not invent an attempted policy"
    );

    let mut missing_with_policy = missing.clone();
    missing_with_policy["attempted_policy_version"] = Value::String("classifier-v1".into());
    assert_rejected(
        &validator,
        &missing_with_policy,
        "project-evidence",
        "missing classification with invented policy",
    );

    let mut empty_relation = missing;
    empty_relation["related_evidence_ids"] = serde_json::json!([]);
    assert_rejected(
        &validator,
        &empty_relation,
        "project-evidence",
        "classification envelope without a source citation",
    );
}
