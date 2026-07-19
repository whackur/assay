use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::ProjectEvidenceManifest;
use crate::RawEvidenceKind;
use crate::machine::codes::raw_issue;
use crate::machine::raw::path_is_publishable;

pub(crate) fn warnings(manifest: &ProjectEvidenceManifest) -> Vec<Value> {
    manifest
        .raw_facts()
        .iter()
        .filter_map(|fact| {
            let issue = match fact.kind() {
                RawEvidenceKind::TrackedFile => {
                    fact.payload().as_tracked_file().and_then(|p| p.issue())
                }
                RawEvidenceKind::HistoryScope => {
                    fact.payload().as_history_scope().and_then(|p| p.issue())
                }
                RawEvidenceKind::ParentDelta => {
                    fact.payload().as_parent_delta().and_then(|p| p.issue())
                }
                RawEvidenceKind::RepositorySnapshot => None,
            }?;
            Some(json!({ "code": raw_issue(issue), "affected_evidence_ids": [fact.id().as_str()] }))
        })
        .collect()
}

pub(crate) fn path_limit_ids(manifest: &ProjectEvidenceManifest) -> Vec<&str> {
    let limited_raw = manifest
        .raw_facts()
        .iter()
        .filter(|fact| {
            fact.source()
                .path()
                .is_some_and(|path| !path_is_publishable(path))
        })
        .map(|fact| fact.id().as_str())
        .collect::<BTreeSet<_>>();
    let mut ids = limited_raw.iter().copied().collect::<BTreeSet<_>>();
    ids.extend(
        manifest
            .classification_facts()
            .iter()
            .filter(|fact| {
                fact.source_evidence_id()
                    .is_some_and(|id| limited_raw.contains(id.as_str()))
            })
            .map(|fact| fact.id().as_str()),
    );
    ids.into_iter().collect()
}

pub(crate) fn is_public_partial_attribute_classification(fact: &Value) -> bool {
    fact["status"] == "partial"
        && fact["payload"]["kind"] == "file_classification"
        && fact["payload"]["reason"] == "attributes_unavailable"
}
