use assay_domain::EvidenceStatus;
use serde_json::{Value, json};

use crate::ClassificationEvidenceRecord;
use crate::machine::codes::{
    category, classification_evidence_kind, classification_reason, evidence_status,
};
use crate::machine::raw::path_is_publishable;
use crate::machine::repository::{map_repository, privacy};

pub(crate) fn map_classification(fact: &ClassificationEvidenceRecord, collected_at: &str) -> Value {
    let related = fact
        .source_evidence_id()
        .map(|id| vec![id.as_str()])
        .unwrap_or_default();
    let mut common = json!({
        "schema_version": "1.0.0",
        "repository": map_repository(fact.source().repository()),
        "id": fact.id().as_str(),
        "status": evidence_status(fact.status()),
        "grade": if matches!(fact.status(), EvidenceStatus::Complete | EvidenceStatus::Partial) { Value::String("a".into()) } else { Value::Null },
        "privacy": privacy(),
        "related_evidence_ids": related
    });
    if let Some(policy_version) = fact.policy_version() {
        common["attempted_policy_version"] = Value::String(policy_version.to_owned());
    }
    let path_limited = fact
        .source()
        .path()
        .is_some_and(|path| !path_is_publishable(path));
    if path_limited && fact.policy_version().is_some() {
        common["status"] = Value::String("unsupported".into());
        common["grade"] = Value::Null;
        common["requested_kind"] = Value::String("file_classification".into());
        common["reason"] = Value::String("path_length_limit".into());
        return common;
    }
    if matches!(
        fact.status(),
        EvidenceStatus::Complete | EvidenceStatus::Partial
    ) {
        let mut value = common;
        value["provenance"] = json!({
            "source_kind": "repository_content",
            "collected_at": collected_at,
            "repository_revision": fact.source().repository_revision().as_str(),
            "content_hash": Value::Null,
            "remote_record_id": Value::Null
        });
        value["payload"] = json!({
            "kind": "file_classification",
            "source_evidence_id": fact.source_evidence_id().map(|id| id.as_str()),
            "policy_version": fact.policy_version(),
            "reason": fact.reason().map(classification_reason),
            "classification": {
                "primary_category": fact.category().map(category),
                "tags": fact.tags().iter().copied().filter_map(crate::machine::codes::tag).collect::<Vec<_>>(),
                "rule_id": fact.rule_id(),
                "confidence": fact.confidence_basis_points().map(|value| f64::from(value) / 10_000.0),
                "evidence": fact.classification_evidence().iter().map(|item| json!({
                    "kind": classification_evidence_kind(item.kind()),
                    "rule_id": item.rule_id(),
                    "attribute_name": item.attribute_name(),
                    "attribute_value": item.attribute_value()
                })).collect::<Vec<_>>()
            }
        });
        value
    } else {
        let mut value = common;
        value["requested_kind"] = Value::String("file_classification".into());
        value["reason"] = Value::String(
            fact.reason()
                .map(classification_reason)
                .unwrap_or("missing_classification")
                .into(),
        );
        value
    }
}
