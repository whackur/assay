use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

mod redundancy;
mod references;
mod status;

pub(crate) use redundancy::repository_feature_id;

use redundancy::{
    validate_evidence_redundancy, validate_history_redundancy, validate_snapshot_redundancy,
};
use references::validate_references;
use status::validate_analysis_status;

/// Enforces cross-component invariants that JSON Schema cannot express.
pub fn validate_project_bundle_consistency(value: &Value) -> Result<(), &'static str> {
    let manifest = value.get("manifest").ok_or("missing_manifest")?;
    let evidence = value
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or("missing_evidence")?;
    let source = &manifest["source_snapshot"]["source"];
    let revision = &manifest["source_snapshot"]["revision"];
    let mut previous: Option<&str> = None;
    let mut by_id = BTreeMap::new();
    for fact in evidence {
        if &fact["repository"] != source {
            return Err("source_mismatch");
        }
        if let Some(provenance) = fact.get("provenance")
            && !provenance["repository_revision"].is_null()
            && &provenance["repository_revision"] != revision
        {
            return Err("revision_mismatch");
        }
        let id = fact["id"].as_str().ok_or("missing_evidence_id")?;
        if previous.is_some_and(|prior| prior >= id) {
            return Err("evidence_order");
        }
        previous = Some(id);
        if by_id.insert(id, fact).is_some() {
            return Err("duplicate_evidence_id");
        }
    }
    validate_references(&by_id, manifest)?;
    validate_evidence_redundancy(&by_id)?;
    validate_snapshot_redundancy(&by_id, manifest, source, revision)?;
    validate_history_redundancy(&by_id, manifest, revision)?;
    let artifacts = manifest["artifacts"].as_array().ok_or("missing_artifact")?;
    let mut matching = artifacts
        .iter()
        .filter(|item| item["role"] == "project_evidence");
    let artifact = matching.next().ok_or("missing_artifact")?;
    if matching.next().is_some() {
        return Err("duplicate_artifact");
    }
    if artifact["status"] != manifest["status"] {
        return Err("artifact_status");
    }
    if artifact["record_count"].as_u64() != Some(evidence.len() as u64) {
        return Err("artifact_count");
    }
    let canonical = serde_json::to_vec(evidence).map_err(|_| "artifact_serialization")?;
    let expected = format!("sha256:{}", hex::encode(Sha256::digest(canonical)));
    if artifact["content_hash"].as_str() != Some(expected.as_str()) {
        return Err("artifact_hash");
    }
    let data_sources = manifest["data_sources"]
        .as_array()
        .ok_or("missing_data_sources")?;
    for source in data_sources {
        if !source["revision"].is_null() && source["revision"] != *revision {
            return Err("data_source_revision");
        }
    }
    validate_analysis_status(manifest, evidence, data_sources, artifacts)?;
    Ok(())
}
