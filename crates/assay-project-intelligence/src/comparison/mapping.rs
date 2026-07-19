use std::collections::BTreeSet;

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource};
use serde_json::{Value, json};

pub(crate) const SCHEMA_VERSION: &str = "1.0.0";

pub(crate) fn basis_points_to_unit(bp: u32) -> f64 {
    f64::from(bp) / 10_000.0
}

pub(crate) fn similarity_value(bp: Option<u32>) -> Value {
    json!({ "status": similarity_status(bp), "value": bp.map(basis_points_to_unit) })
}

pub(crate) fn similarity_status(bp: Option<u32>) -> &'static str {
    if bp.is_some() {
        "complete"
    } else {
        "unavailable"
    }
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

pub(crate) fn source_identifier(source: &RepositorySource) -> String {
    if let Some((provider, namespace, repository)) = source.hosted_locator() {
        format!("{provider}/{namespace}/{repository}")
    } else if let Some(id) = source.local_repository_id() {
        format!("local/{}", id.as_str())
    } else {
        unreachable!("repository source variants are closed")
    }
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

pub(crate) fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
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

pub(crate) fn jaccard_basis_points(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u32 {
    let intersection = left.intersection(right).count() as u64;
    let union = left.union(right).count() as u64;
    if union == 0 {
        return 0;
    }
    ((intersection * 10_000) / union) as u32
}
