use assay_git::RepositorySnapshot;
use serde_json::{Value, json};

use crate::machine::error::MachineMappingError;
use crate::machine::repository::{map_repository, repository_identity_component};

pub(crate) fn repository_features(
    snapshot: &RepositorySnapshot,
    evidence: &[Value],
    collected_at: &str,
) -> Result<Vec<Value>, MachineMappingError> {
    crate::feature::REPOSITORY_FEATURE_NAMES
        .into_iter()
        .map(|feature| {
            let expectation = crate::feature::derive_repository_feature(feature, evidence.iter())
                .map_err(|_| MachineMappingError)?;
            let state = expectation.state;
            let status = if state == "unavailable" {
                "partial"
            } else {
                "complete"
            };
            let identity_scope = repository_identity_component(snapshot.source_snapshot().source());
            let related_ids = expectation.related_evidence_ids;
            let related_refs = related_ids.iter().map(String::as_str).collect::<Vec<_>>();
            let id = crate::contract::repository_feature_id(
                &identity_scope,
                snapshot.source_snapshot().revision().as_str(),
                feature,
                state,
                &related_refs,
            );
            Ok(json!({
                "schema_version": "1.0.0",
                "repository": map_repository(snapshot.source_snapshot().source()),
                "id": id,
                "status": status,
                "grade": "a",
                "privacy": crate::machine::repository::privacy(),
                "provenance": {
                    "source_kind": "repository_content",
                    "collected_at": collected_at,
                    "repository_revision": snapshot.source_snapshot().revision().as_str(),
                    "content_hash": Value::Null,
                    "remote_record_id": Value::Null
                },
                "payload": {
                    "kind": "repository_feature",
                    "feature": feature,
                    "state": state,
                    "related_evidence_ids": related_ids
                }
            }))
        })
        .collect()
}
