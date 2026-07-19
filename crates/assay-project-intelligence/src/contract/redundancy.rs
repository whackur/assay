use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn validate_evidence_redundancy(
    by_id: &BTreeMap<&str, &Value>,
) -> Result<(), &'static str> {
    for fact in by_id.values() {
        let Some(payload) = fact.get("payload") else {
            if fact["requested_kind"] == "file_classification" {
                validate_classification_source(by_id, fact, None)?;
            }
            continue;
        };
        match payload["kind"].as_str() {
            Some("tracked_file") => {
                if fact["provenance"]["content_hash"] != payload["content_hash"] {
                    return Err("tracked_content_hash");
                }
            }
            Some("file_classification") => {
                validate_classification_source(by_id, fact, Some(payload))?;
                if fact["attempted_policy_version"] != payload["policy_version"] {
                    return Err("classification_policy");
                }
            }
            Some("repository_feature") => {
                let state = payload["state"].as_str().ok_or("feature_state")?;
                let status = fact["status"].as_str().ok_or("feature_status")?;
                let related = payload["related_evidence_ids"]
                    .as_array()
                    .ok_or("feature_references")?;
                match state {
                    "present" => {
                        if status != "complete" {
                            return Err("feature_status");
                        }
                        if related.is_empty() {
                            return Err("feature_present_references");
                        }
                    }
                    "unavailable" => {
                        if status != "partial" {
                            return Err("feature_status");
                        }
                        if related.is_empty() {
                            return Err("feature_unavailable_references");
                        }
                    }
                    "absent" => {
                        if status != "complete" {
                            return Err("feature_status");
                        }
                        if !related.is_empty() {
                            return Err("feature_absent_references");
                        }
                    }
                    _ => return Err("feature_state"),
                }
                let related_ids = related
                    .iter()
                    .map(|id| id.as_str().ok_or("feature_reference"))
                    .collect::<Result<Vec<_>, _>>()?;
                if related_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err("feature_reference_order");
                }
                let classification_dependent = !matches!(
                    payload["feature"].as_str(),
                    Some("readme" | "license" | "package_manifest")
                );
                for id in &related_ids {
                    let target = by_id.get(id).ok_or("feature_reference")?;
                    let expected = if classification_dependent {
                        "file_classification"
                    } else {
                        "tracked_file"
                    };
                    let target_kind = target
                        .get("payload")
                        .and_then(|value| value["kind"].as_str())
                        .or_else(|| target["requested_kind"].as_str());
                    if target_kind != Some(expected) {
                        return Err("feature_reference_kind");
                    }
                }
                let identity_scope = repository_identity_component(&fact["repository"])?;
                let revision = fact["provenance"]["repository_revision"]
                    .as_str()
                    .ok_or("feature_revision")?;
                let feature = payload["feature"].as_str().ok_or("feature_name")?;
                let expectation =
                    crate::feature::derive_repository_feature(feature, by_id.values().copied())?;
                if expectation.state != state
                    || expectation.related_evidence_ids
                        != related_ids
                            .iter()
                            .map(|id| (*id).to_owned())
                            .collect::<Vec<_>>()
                {
                    return Err("feature_semantics");
                }
                let expected_id =
                    repository_feature_id(&identity_scope, revision, feature, state, &related_ids);
                if fact["id"].as_str() != Some(expected_id.as_str()) {
                    return Err("feature_identity");
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn repository_feature_id(
    identity_scope: &str,
    revision: &str,
    feature: &str,
    state: &str,
    related_ids: &[&str],
) -> String {
    let id_input = format!(
        "{identity_scope}\0{revision}\0{feature}\0{state}\0{}",
        related_ids.join("\0")
    );
    let digest = hex::encode(Sha256::digest(id_input.as_bytes()));
    format!("evidence:repository-feature:v1-{}", &digest[..24])
}

fn repository_identity_component(source: &Value) -> Result<String, &'static str> {
    match source["kind"].as_str() {
        Some("local") => Ok(format!(
            "local:{}",
            source["repository_id"]
                .as_str()
                .ok_or("feature_repository")?
        )),
        Some("hosted") => Ok(format!(
            "hosted:{}:{}:{}",
            source["provider"].as_str().ok_or("feature_repository")?,
            source["namespace"].as_str().ok_or("feature_repository")?,
            source["repository"].as_str().ok_or("feature_repository")?
        )),
        _ => Err("feature_repository"),
    }
}

fn validate_classification_source(
    by_id: &BTreeMap<&str, &Value>,
    fact: &Value,
    payload: Option<&Value>,
) -> Result<(), &'static str> {
    let related = fact["related_evidence_ids"]
        .as_array()
        .ok_or("classification_relation")?;
    if related.len() != 1 {
        return Err("classification_relation_count");
    }
    let source = payload
        .map(|value| &value["source_evidence_id"])
        .unwrap_or(&related[0]);
    if related[0] != *source {
        return Err("classification_relation");
    }
    let source_id = source.as_str().ok_or("classification_relation")?;
    let target = by_id.get(source_id).ok_or("classification_relation")?;
    let target_kind = target
        .get("payload")
        .and_then(|value| value["kind"].as_str())
        .or_else(|| target["requested_kind"].as_str());
    if target_kind != Some("tracked_file") {
        return Err("classification_source_kind");
    }
    Ok(())
}

pub(super) fn validate_snapshot_redundancy(
    by_id: &BTreeMap<&str, &Value>,
    manifest: &Value,
    source: &Value,
    revision: &Value,
) -> Result<(), &'static str> {
    let snapshots = by_id
        .values()
        .filter(|fact| fact["payload"]["kind"] == "repository_snapshot")
        .copied()
        .collect::<Vec<_>>();
    if snapshots.len() != 1 {
        return Err("repository_snapshot_count");
    }
    let snapshot = snapshots[0];
    if snapshot["repository"] != *source
        || snapshot["provenance"]["repository_revision"] != *revision
        || snapshot["payload"]["root_tree"] != manifest["source_snapshot"]["root_tree"]
        || snapshot["payload"]["commit_time"] != manifest["source_snapshot"]["commit_time"]
    {
        return Err("repository_snapshot_mismatch");
    }
    let data_source = data_source_by_kind(manifest, "repository")?;
    if data_source["id"] != snapshot["id"]
        || data_source["status"] != snapshot["status"]
        || data_source["revision"] != *revision
    {
        return Err("repository_data_source_mismatch");
    }
    Ok(())
}

pub(super) fn validate_history_redundancy(
    by_id: &BTreeMap<&str, &Value>,
    manifest: &Value,
    revision: &Value,
) -> Result<(), &'static str> {
    let histories = by_id
        .values()
        .filter(|fact| {
            fact["payload"]["kind"] == "history_scope" || fact["requested_kind"] == "history_scope"
        })
        .copied()
        .collect::<Vec<_>>();
    if histories.len() != 1 {
        return Err("history_scope_count");
    }
    let history = histories[0];
    let scope = &manifest["scope"];
    if scope["head_revision"] != *revision || scope["history_status"] != history["status"] {
        return Err("history_scope_mismatch");
    }
    if let Some(payload) = history.get("payload") {
        if payload["head_revision"] != scope["head_revision"]
            || payload["base_revision"] != scope["base_revision"]
            || payload["commit_count"] != scope["commit_count"]
        {
            return Err("history_payload_mismatch");
        }
    } else if !scope["commit_count"].is_null() {
        return Err("history_unavailable_count");
    }
    let data_source = data_source_by_kind(manifest, "repository_history")?;
    if data_source["id"] != history["id"]
        || data_source["status"] != history["status"]
        || data_source["revision"] != *revision
    {
        return Err("history_data_source_mismatch");
    }
    Ok(())
}

fn data_source_by_kind<'a>(manifest: &'a Value, kind: &str) -> Result<&'a Value, &'static str> {
    let matches = manifest["data_sources"]
        .as_array()
        .ok_or("missing_data_sources")?
        .iter()
        .filter(|source| source["kind"] == kind)
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        Ok(matches[0])
    } else {
        Err("data_source_kind_count")
    }
}
