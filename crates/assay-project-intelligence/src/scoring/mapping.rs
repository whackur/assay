use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource};
use serde_json::{Value, json};

pub(crate) const SCHEMA_VERSION: &str = "1.0.0";

pub(crate) fn score_value(
    status: EvidenceStatus,
    value: Option<f64>,
    confidence: f64,
    version: &str,
    evidence_ids: &[EvidenceId],
) -> Value {
    json!({
        "status": status_code(status),
        "value": value,
        "confidence": confidence,
        "version": version,
        "evidence_ids": evidence_values(evidence_ids),
    })
}

pub(crate) fn diagnostics(entries: &[(String, Vec<EvidenceId>)]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|(code, evidence_ids)| {
                json!({ "code": code, "evidence_ids": evidence_values(evidence_ids) })
            })
            .collect(),
    )
}

pub(crate) fn evidence_values(evidence_ids: &[EvidenceId]) -> Vec<&str> {
    evidence_ids.iter().map(EvidenceId::as_str).collect()
}

pub(crate) fn repository_value(source: &RepositorySource) -> Value {
    if let Some(id) = source.local_repository_id() {
        json!({ "kind": "local", "repository_id": id.as_str() })
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        json!({ "kind": "hosted", "provider": provider, "namespace": namespace, "repository": repository })
    } else {
        unreachable!("repository source variants are closed")
    }
}

pub(crate) const fn status_code(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}
