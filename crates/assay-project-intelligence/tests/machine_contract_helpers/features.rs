use serde_json::Value;
use sha2::Digest;

use super::ids::repository_feature_id;

pub fn feature<'a>(bundle: &'a Value, name: &str) -> &'a Value {
    bundle["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| fact["payload"]["feature"] == name)
        .unwrap()
}

pub fn feature_related_ids(bundle: &Value, name: &str) -> Vec<String> {
    feature(bundle, name)["payload"]["related_evidence_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect()
}

pub fn set_feature(bundle: &mut Value, name: &str, state: &str, related: &[String]) {
    let related_refs = related.iter().map(String::as_str).collect::<Vec<_>>();
    let id = repository_feature_id(bundle, name, state, &related_refs);
    let fact = bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["feature"] == name)
        .unwrap();
    fact["payload"]["state"] = Value::String(state.into());
    fact["payload"]["related_evidence_ids"] = serde_json::json!(related);
    fact["status"] = Value::String(
        if state == "unavailable" {
            "partial"
        } else {
            "complete"
        }
        .into(),
    );
    fact["id"] = Value::String(id);
    let analysis_status = if state == "unavailable" {
        "partial"
    } else {
        "complete"
    };
    bundle["manifest"]["status"] = Value::String(analysis_status.into());
    refresh_project_artifact(bundle);
    bundle["manifest"]["artifacts"][0]["status"] = Value::String(analysis_status.into());
}

pub fn feature_mut(bundle: &mut Value) -> &mut Value {
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "repository_feature")
        .unwrap()
}

pub fn refresh_project_artifact(bundle: &mut Value) {
    let evidence = bundle["evidence"].as_array_mut().unwrap();
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let bytes = serde_json::to_vec(evidence).unwrap();
    bundle["manifest"]["artifacts"][0]["record_count"] = Value::from(evidence.len());
    bundle["manifest"]["artifacts"][0]["content_hash"] = Value::String(format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(bytes))
    ));
}
