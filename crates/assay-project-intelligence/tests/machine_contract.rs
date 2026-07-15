use assay_project_intelligence::validate_project_bundle_consistency;
use serde_json::Value;

fn coherent_bundle() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/golden/project-analysis-v1.json"
    ))
    .expect("reviewed project-analysis golden must parse")
}

#[test]
fn public_contract_validator_accepts_the_reviewed_coherent_bundle() {
    validate_project_bundle_consistency(&coherent_bundle())
        .expect("reviewed project-analysis golden must be coherent");
}

#[test]
fn public_contract_validator_rejects_a_dangling_data_source() {
    let mut bundle = coherent_bundle();
    bundle["manifest"]["data_sources"][0]["id"] =
        Value::String("evidence:missing:data-source".into());
    assert_eq!(
        validate_project_bundle_consistency(&bundle),
        Err("unknown_evidence_reference")
    );
}
