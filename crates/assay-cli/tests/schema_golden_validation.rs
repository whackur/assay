mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{
    contracts, parse_json_without_duplicate_keys, read_json, repository_root, validation_messages,
    validator,
};

#[test]
fn reviewed_golden_contracts_validate() {
    for contract in contracts() {
        let validator = validator(&contract.name);
        let instance = read_json(contract.golden.clone());
        let errors = validation_messages(&validator, &instance);
        assert!(
            errors.is_empty(),
            "{} failed {}: {errors:#?}",
            contract.golden.display(),
            contract.name
        );
    }
}

#[test]
fn discovered_contract_files_have_exact_fixture_mapping_and_unique_json_keys() {
    assert_eq!(contracts().len(), 8);
    assert!(
        parse_json_without_duplicate_keys(r#"{"schema_version":"1.0.0","schema_version":"1.0.1"}"#)
            .is_err(),
        "duplicate JSON keys must never be silently overwritten"
    );
}

#[test]
fn reviewed_invalid_fixtures_are_rejected() {
    for contract in contracts() {
        let validator = validator(&contract.name);
        for fixture in contract.invalid_fixtures {
            let fixture_name = fixture
                .file_name()
                .and_then(|value| value.to_str())
                .expect("invalid fixture must have a UTF-8 file name");
            let mut instance = read_json(fixture.clone());
            schema_contracts_helpers::common::assert_rejected(
                &validator,
                &instance,
                &contract.name,
                fixture_name,
            );

            match fixture_name {
                "analysis-manifest-v1-unknown-field.json" => {
                    instance
                        .as_object_mut()
                        .expect("analysis manifest must be an object")
                        .remove("contributor_score");
                }
                "project-evidence-v1-absolute-path.json" => {
                    instance["payload"]["relative_path"] = Value::String("src/main.rs".to_owned());
                }
                "ai-judgment-v1-missing-citation.json" => {
                    instance["judgments"][0]["evidence_ids"] =
                        serde_json::json!(["evidence:repository:snapshot"]);
                }
                "project-evaluation-v1-person-score.json" => {
                    instance["scores"]
                        .as_object_mut()
                        .expect("scores must be an object")
                        .remove("person_performance");
                }
                "capabilities-v1-future-claim.json" => {
                    instance["commands"] = serde_json::json!(["capabilities", "project analyze"]);
                }
                "project-analysis-v1-invalid-nested-manifest.json" => {
                    instance = schema_contracts_helpers::common::golden("project-analysis");
                }
                "project-comparison-v1-recursive-depth.json" => {
                    instance["search_depth"] = Value::String("one_depth".to_owned());
                }
                "project-comparison-v1-uncited-selection.json" => {
                    instance["detailed_candidates"][0]["selection_reasons"] =
                        serde_json::json!(["technical_similarity"]);
                }
                "run-state-v1-complete-stage-with-reason.json" => {
                    let stages = instance["stages"].as_array_mut().unwrap();
                    let stage = stages
                        .iter_mut()
                        .find(|entry| entry["status"] == "complete")
                        .unwrap();
                    stage["reason"] = Value::Null;
                }
                _ => panic!("invalid fixture lacks an isolation repair: {fixture_name}"),
            }
            let errors = validation_messages(&validator, &instance);
            assert!(
                errors.is_empty(),
                "repairing {fixture_name} did not isolate its intended failure: {errors:#?}"
            );
        }
    }
}

#[test]
fn public_schemas_are_closed_and_use_only_internal_or_bundled_references() {
    for contract in contracts() {
        let schema = read_json(
            repository_root()
                .join("schemas")
                .join(&contract.name)
                .join("v1.json"),
        );
        schema_contracts_helpers::assertions::assert_closed_objects_and_bundled_refs(
            &schema,
            &schema,
            &contract.name,
        );
    }
}
