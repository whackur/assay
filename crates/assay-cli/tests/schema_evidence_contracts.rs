mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{
    assert_rejected, golden, read_json, repository_root, validation_messages, validator,
};
use schema_contracts_helpers::fixtures::tracked_file_record;

#[test]
fn unavailable_project_evidence_is_an_availability_envelope_not_a_fact() {
    let root = repository_root();
    let validator = validator("project-evidence");
    let complete = read_json(root.join("tests/golden/project-evidence-v1.json"));
    let mut unavailable = complete.clone();
    unavailable["grade"] = Value::Null;
    unavailable
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("provenance");
    unavailable
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("payload");
    unavailable["requested_kind"] = Value::String("file".to_owned());
    unavailable["reason"] = Value::String("source_unavailable".to_owned());
    for status in ["unavailable", "unsupported", "insufficient", "pending"] {
        let mut envelope = unavailable.clone();
        envelope["status"] = Value::String(status.to_owned());
        let errors = validation_messages(&validator, &envelope);
        assert!(
            errors.is_empty(),
            "{status} availability envelope must validate: {errors:#?}"
        );
    }
    unavailable["status"] = Value::String("unavailable".to_owned());

    let mut partial = complete.clone();
    partial["status"] = Value::String("partial".to_owned());
    let errors = validation_messages(&validator, &partial);
    assert!(
        errors.is_empty(),
        "partial factual evidence must validate: {errors:#?}"
    );

    let mut contradictory = unavailable.clone();
    contradictory["provenance"] = complete["provenance"].clone();
    contradictory["payload"] = complete["payload"].clone();
    assert_rejected(
        &validator,
        &contradictory,
        "project-evidence",
        "unavailable evidence with factual payload",
    );

    let mut graded_unavailable = unavailable.clone();
    graded_unavailable["grade"] = Value::String("a".to_owned());
    assert_rejected(
        &validator,
        &graded_unavailable,
        "project-evidence",
        "unavailable evidence with grade",
    );

    let mut factless_complete = unavailable.clone();
    factless_complete["status"] = Value::String("complete".to_owned());
    factless_complete["grade"] = Value::String("a".to_owned());
    assert_rejected(
        &validator,
        &factless_complete,
        "project-evidence",
        "complete evidence without factual payload",
    );

    let mut incomplete_fact = complete;
    incomplete_fact
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("provenance");
    assert_rejected(
        &validator,
        &incomplete_fact,
        "project-evidence",
        "complete evidence without immutable provenance",
    );
}

#[test]
fn new_raw_and_derived_file_contracts_preserve_partial_state() {
    let validator = validator("project-evidence");
    let mut tracked = golden("project-evidence");
    tracked["status"] = Value::String("partial".into());
    tracked["payload"] = serde_json::json!({
        "kind": "tracked_file",
        "path": { "encoding": "utf8", "value": "src/main.ts" },
        "mode": "regular",
        "object_kind": "blob",
        "object_id": "89abcdef0123456789abcdef0123456789abcdef",
        "content_status": "partial",
        "language": "TypeScript",
        "language_status": "complete",
        "size_bytes": 20_000_000,
        "content_hash": null,
        "issue": "size_limit"
    });
    assert!(validator.is_valid(&tracked));

    let mut fake_complete = tracked.clone();
    fake_complete["payload"]["content_status"] = Value::String("complete".into());
    assert_rejected(
        &validator,
        &fake_complete,
        "project-evidence",
        "complete content without hash",
    );

    let mut fake_unavailable = tracked.clone();
    fake_unavailable["payload"]["content_status"] = Value::String("unavailable".into());
    fake_unavailable["payload"]["size_bytes"] = Value::from(0);
    fake_unavailable["payload"]["content_hash"] =
        Value::String(format!("sha256:{}", "0".repeat(64)));
    fake_unavailable["payload"]["issue"] = Value::String("timeout".into());
    assert_rejected(
        &validator,
        &fake_unavailable,
        "project-evidence",
        "unavailable content with fabricated values",
    );

    let mut gitlink = tracked.clone();
    gitlink["payload"] = serde_json::json!({
        "kind": "tracked_file",
        "path": { "encoding": "git_path_hex", "value": "7375626d6f64756c65" },
        "mode": "gitlink",
        "object_kind": "commit",
        "object_id": "89abcdef0123456789abcdef0123456789abcdef",
        "content_status": "unsupported",
        "language": null,
        "language_status": "unsupported",
        "size_bytes": null,
        "content_hash": null,
        "issue": "gitlink_content"
    });
    assert!(validator.is_valid(&gitlink));
    gitlink["payload"]["object_kind"] = Value::String("blob".into());
    assert_rejected(
        &validator,
        &gitlink,
        "project-evidence",
        "gitlink blob contradiction",
    );

    let mut classification = golden("project-evidence");
    classification["status"] = Value::String("partial".into());
    classification["related_evidence_ids"] = serde_json::json!(["evidence:file:src-main-ts"]);
    classification["attempted_policy_version"] = Value::String("classifier-v1".into());
    classification["payload"] = serde_json::json!({
        "kind": "file_classification",
        "source_evidence_id": "evidence:file:src-main-ts",
        "policy_version": "classifier-v1",
        "reason": "attributes_unavailable",
        "classification": {
            "primary_category": "production_code",
            "tags": ["production"],
            "rule_id": "path.production.typescript",
            "confidence": 1.0,
            "evidence": [{
                "kind": "attribute_facts_unavailable",
                "rule_id": "attributes.unavailable",
                "attribute_name": null,
                "attribute_value": null
            }]
        }
    });
    assert!(validator.is_valid(&classification));
    classification["payload"]["reason"] = Value::Null;
    assert_rejected(
        &validator,
        &classification,
        "project-evidence",
        "partial classification without reason",
    );
}

#[test]
fn tracked_language_presence_matches_its_supported_status() {
    let validator = validator("project-evidence");
    let mut tracked = golden("project-evidence");
    tracked["payload"] = tracked_file_record()["payload"].clone();
    assert!(validator.is_valid(&tracked));

    let mut named_but_unsupported = tracked.clone();
    named_but_unsupported["payload"]["language_status"] = Value::String("unsupported".into());
    assert_rejected(
        &validator,
        &named_but_unsupported,
        "project-evidence",
        "named language with unsupported status",
    );

    let mut unnamed_but_complete = tracked;
    unnamed_but_complete["payload"]["language"] = Value::Null;
    assert_rejected(
        &validator,
        &unnamed_but_complete,
        "project-evidence",
        "null language with complete status",
    );
}
