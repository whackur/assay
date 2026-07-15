use std::{fs, path::PathBuf};

use jsonschema::{Draft, Validator};
use serde_json::Value;

const CONTRACTS: [(&str, &str); 4] = [
    ("analysis-manifest", "analysis-manifest-v1.json"),
    ("project-evidence", "project-evidence-v1.json"),
    ("ai-judgment", "ai-judgment-v1.json"),
    ("project-evaluation", "project-evaluation-v1.json"),
];

const INVALID_FIXTURES: [(&str, &str); 4] = [
    (
        "analysis-manifest",
        "analysis-manifest-v1-unknown-field.json",
    ),
    ("project-evidence", "project-evidence-v1-absolute-path.json"),
    ("ai-judgment", "ai-judgment-v1-missing-citation.json"),
    (
        "project-evaluation",
        "project-evaluation-v1-person-score.json",
    ),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

fn read_json(path: PathBuf) -> Value {
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("invalid JSON in {}: {error}", path.display()))
}

fn validator(contract: &str) -> Validator {
    let schema = read_json(
        repository_root()
            .join("schemas")
            .join(contract)
            .join("v1.json"),
    );
    jsonschema::draft202012::meta::validate(&schema)
        .unwrap_or_else(|error| panic!("{contract} failed the Draft 2020-12 meta-schema: {error}"));
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(&schema)
        .unwrap_or_else(|error| panic!("invalid {contract} schema: {error}"))
}

fn validation_messages(validator: &Validator, instance: &Value) -> Vec<String> {
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn reviewed_golden_contracts_validate() {
    let root = repository_root();

    for (contract, fixture) in CONTRACTS {
        let validator = validator(contract);
        let instance = read_json(root.join("tests/golden").join(fixture));
        let errors = validation_messages(&validator, &instance);
        assert!(
            errors.is_empty(),
            "{fixture} failed {contract}: {errors:#?}"
        );
    }
}

#[test]
fn later_v1_instance_versions_preserve_the_declared_major_contract() {
    let root = repository_root();

    for (contract, fixture) in CONTRACTS {
        let validator = validator(contract);
        let mut instance = read_json(root.join("tests/golden").join(fixture));
        instance["schema_version"] = Value::String("1.99.0".to_owned());
        let errors = validation_messages(&validator, &instance);
        assert!(
            errors.is_empty(),
            "{contract} rejected a later v1 instance version: {errors:#?}"
        );
    }
}

#[test]
fn representative_invalid_contracts_are_rejected() {
    let root = repository_root();

    for (contract, fixture) in CONTRACTS {
        let validator = validator(contract);
        let instance = read_json(root.join("tests/golden").join(fixture));

        let mut missing_required = instance.clone();
        missing_required
            .as_object_mut()
            .expect("golden contract must be an object")
            .remove("schema_version");
        assert_rejected(
            &validator,
            &missing_required,
            contract,
            "missing required field",
        );

        let mut unknown_field = instance.clone();
        unknown_field
            .as_object_mut()
            .expect("golden contract must be an object")
            .insert("undocumented_field".to_owned(), Value::Bool(true));
        assert_rejected(&validator, &unknown_field, contract, "unknown field");

        let mut next_major = instance.clone();
        next_major["schema_version"] = Value::String("2.0.0".to_owned());
        assert_rejected(&validator, &next_major, contract, "next major version");

        let mut unknown_status = instance;
        unknown_status["status"] = Value::String("unknown".to_owned());
        assert_rejected(&validator, &unknown_status, contract, "unknown status");
    }
}

#[test]
fn reviewed_invalid_fixtures_are_rejected() {
    let root = repository_root();

    for (contract, fixture) in INVALID_FIXTURES {
        let validator = validator(contract);
        let mut instance = read_json(root.join("tests/fixtures/schema-invalid").join(fixture));
        assert_rejected(&validator, &instance, contract, fixture);

        match contract {
            "analysis-manifest" => {
                instance
                    .as_object_mut()
                    .expect("analysis manifest must be an object")
                    .remove("contributor_score");
            }
            "project-evidence" => {
                instance["payload"]["relative_path"] = Value::String("src/main.rs".to_owned());
            }
            "ai-judgment" => {
                instance["judgments"][0]["evidence_ids"] =
                    serde_json::json!(["evidence:repository:snapshot"]);
            }
            "project-evaluation" => {
                instance["scores"]
                    .as_object_mut()
                    .expect("scores must be an object")
                    .remove("person_performance");
            }
            _ => unreachable!("all invalid fixture contracts are enumerated"),
        }
        let errors = validation_messages(&validator, &instance);
        assert!(
            errors.is_empty(),
            "repairing {fixture} did not isolate its intended failure: {errors:#?}"
        );
    }
}

#[test]
fn privacy_citation_and_product_domain_boundaries_are_enforced() {
    let root = repository_root();

    let evidence_validator = validator("project-evidence");
    let mut evidence = read_json(root.join("tests/golden/project-evidence-v1.json"));
    evidence["payload"]["relative_path"] = Value::String("/private/source.rs".to_owned());
    assert_rejected(
        &evidence_validator,
        &evidence,
        "project-evidence",
        "absolute source path",
    );

    let mut private_evidence = read_json(root.join("tests/golden/project-evidence-v1.json"));
    private_evidence["privacy"]["visibility"] = Value::String("private_local".to_owned());
    assert_rejected(
        &evidence_validator,
        &private_evidence,
        "project-evidence",
        "private evidence without an explicit transmission boundary",
    );

    for unsafe_path in ["../source.rs", "C:\\private\\source.rs"] {
        let mut unsafe_evidence = read_json(root.join("tests/golden/project-evidence-v1.json"));
        unsafe_evidence["payload"]["relative_path"] = Value::String(unsafe_path.to_owned());
        assert_rejected(
            &evidence_validator,
            &unsafe_evidence,
            "project-evidence",
            "non-portable source path",
        );
    }

    let judgment_validator = validator("ai-judgment");
    let mut judgment = read_json(root.join("tests/golden/ai-judgment-v1.json"));
    judgment["judgments"][0]["evidence_ids"] = Value::Array(Vec::new());
    assert_rejected(
        &judgment_validator,
        &judgment,
        "ai-judgment",
        "uncited applicable judgment",
    );

    let mut out_of_range = read_json(root.join("tests/golden/ai-judgment-v1.json"));
    out_of_range["judgments"][0]["rating"] = Value::Number(5.into());
    assert_rejected(
        &judgment_validator,
        &out_of_range,
        "ai-judgment",
        "out-of-range rating",
    );

    let evaluation_validator = validator("project-evaluation");
    let mut evaluation = read_json(root.join("tests/golden/project-evaluation-v1.json"));
    evaluation["scores"]
        .as_object_mut()
        .expect("scores must be an object")
        .insert("person_performance".to_owned(), Value::Null);
    assert_rejected(
        &evaluation_validator,
        &evaluation,
        "project-evaluation",
        "person-level score",
    );

    let mut numeric_without_evidence =
        read_json(root.join("tests/golden/project-evaluation-v1.json"));
    numeric_without_evidence["scores"]["project_substance"]["status"] =
        Value::String("complete".to_owned());
    numeric_without_evidence["scores"]["project_substance"]["value"] = Value::Number(80.into());
    assert_rejected(
        &evaluation_validator,
        &numeric_without_evidence,
        "project-evaluation",
        "numeric score without evidence citations",
    );

    let manifest_validator = validator("analysis-manifest");
    let mut false_complete = read_json(root.join("tests/golden/analysis-manifest-v1.json"));
    false_complete["status"] = Value::String("complete".to_owned());
    assert_rejected(
        &manifest_validator,
        &false_complete,
        "analysis-manifest",
        "complete run with partial evidence",
    );
}

fn assert_rejected(validator: &Validator, instance: &Value, contract: &str, case: &str) {
    assert!(
        !validator.is_valid(instance),
        "{case} unexpectedly satisfied {contract}"
    );
}

#[test]
fn public_schemas_are_closed_and_use_only_internal_references() {
    for (contract, _) in CONTRACTS {
        let schema = read_json(
            repository_root()
                .join("schemas")
                .join(contract)
                .join("v1.json"),
        );
        assert_closed_objects_and_internal_refs(&schema, contract);
    }
}

fn assert_closed_objects_and_internal_refs(value: &Value, location: &str) {
    match value {
        Value::Object(object) => {
            if object.get("type") == Some(&Value::String("object".to_owned())) {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "object schema at {location} must declare additionalProperties: false"
                );
            }
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                assert!(
                    reference.starts_with("#/$defs/"),
                    "external or non-canonical reference at {location}: {reference}"
                );
            }
            for (key, child) in object {
                assert_closed_objects_and_internal_refs(child, &format!("{location}/{key}"));
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                assert_closed_objects_and_internal_refs(child, &format!("{location}/{index}"));
            }
        }
        _ => {}
    }
}
