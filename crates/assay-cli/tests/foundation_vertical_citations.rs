#![cfg(unix)]
//! Citation audit rejects removed, nested, and manifest branches.

mod foundation_vertical;

use foundation_vertical::fixture::FoundationFixture;
use foundation_vertical::runner::run_analysis;
use foundation_vertical::validation::audit_bundle_citations;

use serde_json::Value;

#[test]
fn citation_audit_rejects_removed_nested_and_manifest_branches() {
    let fixture = FoundationFixture::build();
    let output = run_analysis(&fixture);
    assert_eq!(output.status.code(), Some(0));
    let bundle: Value = serde_json::from_slice(&output.stdout).expect("analysis bundle");

    let mut removed_source = bundle.clone();
    let classification = removed_source["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| record["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["payload"]
        .as_object_mut()
        .unwrap()
        .remove("source_evidence_id");
    assert!(audit_bundle_citations(&removed_source).is_err());

    let mut emptied_feature = bundle.clone();
    let feature = emptied_feature["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|record| {
            record["payload"]["kind"] == "repository_feature"
                && !record["payload"]["related_evidence_ids"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        })
        .unwrap();
    feature["payload"]["related_evidence_ids"] = Value::Array(Vec::new());
    assert!(audit_bundle_citations(&emptied_feature).is_err());

    let mut removed_limitation = bundle;
    removed_limitation["manifest"]["limitations"][0]
        .as_object_mut()
        .unwrap()
        .remove("affected_evidence_ids");
    assert!(audit_bundle_citations(&removed_limitation).is_err());
}
