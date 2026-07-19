use serde_json::Value;

pub fn tracked_file_record() -> Value {
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
            "path": { "encoding": "utf8", "value": "src/main.ts" },
            "mode": "regular",
            "object_kind": "blob",
            "object_id": "89abcdef0123456789abcdef0123456789abcdef",
            "content_status": "complete",
            "language": "TypeScript",
            "language_status": "complete",
            "size_bytes": 418,
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "issue": null
        }
    })
}

pub fn classification_record() -> Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "repository": {
            "kind": "local",
            "repository_id": format!("sha256:{}", "1".repeat(64))
        },
        "id": "evidence:file-classification:v1-golden",
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "related_evidence_ids": ["evidence:tracked-file:v1-golden"],
        "attempted_policy_version": "classifier-v1",
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": "0123456789abcdef0123456789abcdef01234567",
            "content_hash": null,
            "remote_record_id": null
        },
        "payload": {
            "kind": "file_classification",
            "source_evidence_id": "evidence:tracked-file:v1-golden",
            "policy_version": "classifier-v1",
            "reason": null,
            "classification": {
                "primary_category": "production_code",
                "tags": [],
                "rule_id": "path.production.typescript",
                "confidence": 1.0,
                "evidence": [{
                    "kind": "policy_rule",
                    "rule_id": "path.production.typescript",
                    "attribute_name": null,
                    "attribute_value": null
                }]
            }
        }
    })
}

pub fn repository_feature_record(status: &str, state: &str, related: Value) -> Value {
    let mut feature = tracked_file_record();
    feature["id"] = Value::String("evidence:repository-feature:v1-golden".into());
    feature["status"] = Value::String(status.into());
    feature["provenance"]["content_hash"] = Value::Null;
    feature["payload"] = serde_json::json!({
        "kind": "repository_feature",
        "feature": "readme",
        "state": state,
        "related_evidence_ids": related
    });
    feature
}
