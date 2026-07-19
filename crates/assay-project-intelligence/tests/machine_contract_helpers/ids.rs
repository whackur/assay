use sha2::{Digest, Sha256};

pub fn repository_feature_id(
    bundle: &serde_json::Value,
    feature: &str,
    state: &str,
    related: &[&str],
) -> String {
    let source = &bundle["manifest"]["source_snapshot"]["source"];
    let identity_scope = format!("local:{}", source["repository_id"].as_str().unwrap());
    let revision = bundle["manifest"]["source_snapshot"]["revision"]
        .as_str()
        .unwrap();
    let input = format!(
        "{identity_scope}\0{revision}\0{feature}\0{state}\0{}",
        related.join("\0")
    );
    let digest = hex::encode(Sha256::digest(input.as_bytes()));
    format!("evidence:repository-feature:v1-{}", &digest[..24])
}
