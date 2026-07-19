use std::collections::BTreeMap;

use assay_git::RepositorySnapshot;
use serde_json::{Value, json};

use crate::ProjectEvidenceManifest;
use crate::RawEvidenceKind;
use crate::machine::classification::map_classification;
use crate::machine::codes::analysis_status;
use crate::machine::diagnostics::{
    is_public_partial_attribute_classification, path_limit_ids, warnings,
};
use crate::machine::error::MachineMappingError;
use crate::machine::features::repository_features;
use crate::machine::hash::{sha256, stable_hash};
use crate::machine::raw::map_raw;
use crate::machine::repository::map_repository;

/// Maps shared immutable facts into the reviewed public CLI bundle.
///
/// The supplied timestamp is a delivery-boundary clock value. No source bytes,
/// raw diffs, host paths, credentials, person observations, or scores are
/// retained.
pub fn build_project_analysis(
    snapshot: &RepositorySnapshot,
    manifest: &ProjectEvidenceManifest,
    generated_at: &str,
) -> Result<Value, MachineMappingError> {
    let classifications = manifest
        .classification_facts()
        .iter()
        .filter_map(|fact| fact.source_evidence_id().map(|id| (id.as_str(), fact)))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::new();
    for raw in manifest.raw_facts() {
        evidence.push(map_raw(snapshot, raw, generated_at)?);
        if raw.kind() == RawEvidenceKind::TrackedFile
            && let Some(classification) = classifications.get(raw.id().as_str())
        {
            evidence.push(map_classification(classification, generated_at));
        }
    }
    evidence.extend(repository_features(snapshot, &evidence, generated_at)?);
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));

    let evidence_bytes = serde_json::to_vec(&evidence).map_err(|_| MachineMappingError)?;
    let artifact_hash = sha256(&evidence_bytes);
    let snapshot_id = manifest
        .raw_facts()
        .iter()
        .find(|fact| fact.kind() == RawEvidenceKind::RepositorySnapshot)
        .map(|fact| fact.id().as_str())
        .ok_or(MachineMappingError)?;
    let history = manifest
        .raw_facts()
        .iter()
        .find(|fact| fact.kind() == RawEvidenceKind::HistoryScope)
        .ok_or(MachineMappingError)?;
    let history_payload = history
        .payload()
        .as_history_scope()
        .ok_or(MachineMappingError)?;
    let has_bounded_path = manifest.raw_facts().iter().any(|fact| {
        fact.source()
            .path()
            .is_some_and(|path| !crate::machine::raw::path_is_publishable(path))
    });
    let analysis_status_value = if has_bounded_path {
        "partial"
    } else {
        analysis_status(manifest.status())
    };
    let revision = snapshot.source_snapshot().revision().as_str();
    let source = map_repository(snapshot.source_snapshot().source());
    let warnings = warnings(manifest);
    let path_limit_ids = path_limit_ids(manifest);
    let attribute_unavailable_ids = evidence
        .iter()
        .filter(|fact| is_public_partial_attribute_classification(fact))
        .filter_map(|fact| fact["id"].as_str())
        .collect::<Vec<_>>();
    let mut limitations = Vec::new();
    if !attribute_unavailable_ids.is_empty() {
        limitations.push(json!({
            "code": "attribute_resolution_unavailable",
            "affected_evidence_ids": attribute_unavailable_ids
        }));
    }
    limitations.extend([
        json!({ "code": "project_scores_not_computed", "affected_evidence_ids": [snapshot_id] }),
        json!({ "code": "repository_code_not_executed", "affected_evidence_ids": [snapshot_id] }),
    ]);
    if !path_limit_ids.is_empty() {
        limitations
            .push(json!({ "code": "path_length_limit", "affected_evidence_ids": path_limit_ids }));
    }
    let manifest_value = json!({
        "schema_version": "1.0.0",
        "analysis_version": "repository-evidence-1",
        "tool": { "name": "assay", "version": env!("CARGO_PKG_VERSION") },
        "source_snapshot": {
            "source": source,
            "revision": revision,
            "root_tree": snapshot.source_snapshot().root_tree().map(|value| value.as_str()),
            "commit_time": snapshot.commit_time()
        },
        "rule_set_hash": stable_hash(b"assay-rule-set-v1\0classifier-v1\0project-evidence-v1"),
        "config_hash": stable_hash(b"assay-effective-config-v1\0default-local-read-only"),
        "analyzers": [
            { "name": "assay-classifier", "version": env!("CARGO_PKG_VERSION") },
            { "name": "assay-git", "version": env!("CARGO_PKG_VERSION") },
            { "name": "assay-project-intelligence", "version": env!("CARGO_PKG_VERSION") }
        ],
        "parsers": [],
        "status": analysis_status_value,
        "generated_at": generated_at,
        "scope": {
            "mode": "single_revision",
            "base_revision": Value::Null,
            "head_revision": revision,
            "history_status": crate::machine::codes::evidence_status(history.status()),
            "commit_count": history_payload.reachable_commits(),
            "requested_capabilities": [
                "repository_snapshot", "tracked_files", "file_classification",
                "repository_history", "language_detection"
            ]
        },
        "data_sources": [
            {
                "id": snapshot_id,
                "kind": "repository",
                "status": crate::machine::codes::evidence_status(snapshot.status()),
                "revision": revision,
                "content_hash": Value::Null,
                "remote_record_id": Value::Null,
                "collected_at": generated_at,
                "visibility": "private_local",
                "retention": "metadata_only"
            },
            {
                "id": history.id().as_str(),
                "kind": "repository_history",
                "status": crate::machine::codes::evidence_status(history.status()),
                "revision": revision,
                "content_hash": Value::Null,
                "remote_record_id": Value::Null,
                "collected_at": generated_at,
                "visibility": "private_local",
                "retention": "metadata_only"
            }
        ],
        "artifacts": [{
            "role": "project_evidence",
            "schema_version": "1.0.0",
            "content_hash": artifact_hash,
            "record_count": evidence.len(),
            "status": analysis_status_value
        }],
        "warnings": warnings,
        "limitations": limitations
    });
    Ok(json!({
        "schema_version": "1.0.0",
        "manifest": manifest_value,
        "evidence": evidence
    }))
}
