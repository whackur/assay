use std::{fmt, fs, path::PathBuf};

use jsonschema::{Draft, Validator};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;

#[derive(Debug)]
struct Contract {
    name: String,
    golden: PathBuf,
    invalid_fixtures: Vec<PathBuf>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

fn contracts() -> Vec<Contract> {
    let root = repository_root();
    let schemas = root.join("schemas");
    let invalid_root = root.join("tests/fixtures/schema-invalid");
    let mut names = fs::read_dir(&schemas)
        .unwrap_or_else(|error| panic!("failed to discover {}: {error}", schemas.display()))
        .filter_map(|entry| {
            let entry = entry.expect("schema directory entry must be readable");
            entry
                .file_type()
                .expect("schema entry type must be readable")
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| entry.path().join("v1.json").is_file() && !name.starts_with('.'))
        })
        .collect::<Vec<_>>();
    names.sort();
    assert!(
        !names.is_empty(),
        "at least one public schema must be discovered"
    );

    let invalid_paths = fs::read_dir(&invalid_root)
        .unwrap_or_else(|error| panic!("failed to discover {}: {error}", invalid_root.display()))
        .filter_map(|entry| {
            let path = entry
                .expect("invalid fixture entry must be readable")
                .path();
            (path.extension().and_then(|value| value.to_str()) == Some("json")).then_some(path)
        })
        .collect::<Vec<_>>();

    let mut golden_names = fs::read_dir(root.join("tests/golden"))
        .expect("golden directory must be readable")
        .filter_map(|entry| {
            let file_name = entry
                .expect("golden entry must be readable")
                .file_name()
                .to_string_lossy()
                .into_owned();
            file_name.strip_suffix("-v1.json").map(str::to_owned)
        })
        .collect::<Vec<_>>();
    golden_names.sort();
    assert_eq!(
        names, golden_names,
        "every discovered v1 schema must have exactly one matching golden and no orphan golden"
    );

    let contracts = names
        .into_iter()
        .map(|name| {
            let golden = root.join("tests/golden").join(format!("{name}-v1.json"));
            assert!(
                golden.is_file(),
                "missing golden for discovered schema {name}"
            );
            let prefix = format!("{name}-v1-");
            let mut invalid_fixtures = invalid_paths
                .iter()
                .filter(|path| {
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|file_name| file_name.starts_with(&prefix))
                })
                .cloned()
                .collect::<Vec<_>>();
            invalid_fixtures.sort();
            assert!(
                !invalid_fixtures.is_empty(),
                "missing invalid fixture for discovered schema {name}"
            );
            Contract {
                name,
                golden,
                invalid_fixtures,
            }
        })
        .collect::<Vec<_>>();
    let mapped_invalid_count = contracts
        .iter()
        .map(|contract| contract.invalid_fixtures.len())
        .sum::<usize>();
    assert_eq!(
        mapped_invalid_count,
        invalid_paths.len(),
        "every invalid fixture must map to exactly one discovered v1 schema"
    );
    contracts
}

fn read_json(path: PathBuf) -> Value {
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    parse_json_without_duplicate_keys(&contents)
        .unwrap_or_else(|error| panic!("invalid JSON in {}: {error}", path.display()))
}

fn parse_json_without_duplicate_keys(contents: &str) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_str(contents);
    let value = UniqueJson::deserialize(&mut deserializer)
        .map_err(|error| error.to_string())?
        .0;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(value)
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueJson>()? {
            values.push(value.0);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key: {key}")));
            }
            values.insert(key, map.next_value::<UniqueJson>()?.0);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
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
        .should_validate_formats(true)
        .build(&schema)
        .unwrap_or_else(|error| panic!("invalid {contract} schema: {error}"))
}

fn validation_messages(validator: &Validator, instance: &Value) -> Vec<String> {
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

fn golden(contract: &str) -> Value {
    read_json(
        repository_root()
            .join("tests/golden")
            .join(format!("{contract}-v1.json")),
    )
}

fn assert_golden_mutation_rejected(contract: &str, case: &str, mutate: impl FnOnce(&mut Value)) {
    let validator = validator(contract);
    let mut instance = golden(contract);
    mutate(&mut instance);
    assert_rejected(&validator, &instance, contract, case);
}

fn assert_golden_value_rejected(contract: &str, pointer: &str, value: Value, case: &str) {
    assert_golden_mutation_rejected(contract, case, |instance| {
        *instance
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("missing golden pointer {contract}{pointer}")) = value;
    });
}

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
fn later_v1_instance_versions_preserve_the_declared_major_contract() {
    for contract in contracts() {
        let validator = validator(&contract.name);
        let mut instance = read_json(contract.golden);
        instance["schema_version"] = Value::String("1.99.0".to_owned());
        let errors = validation_messages(&validator, &instance);
        assert!(
            errors.is_empty(),
            "{} rejected a later v1 instance version: {errors:#?}",
            contract.name
        );
    }
}

#[test]
fn representative_invalid_contracts_are_rejected() {
    for contract in contracts() {
        let validator = validator(&contract.name);
        let instance = read_json(contract.golden);

        let mut missing_required = instance.clone();
        missing_required
            .as_object_mut()
            .expect("golden contract must be an object")
            .remove("schema_version");
        assert_rejected(
            &validator,
            &missing_required,
            &contract.name,
            "missing required field",
        );

        let mut unknown_field = instance.clone();
        unknown_field
            .as_object_mut()
            .expect("golden contract must be an object")
            .insert("undocumented_field".to_owned(), Value::Bool(true));
        assert_rejected(&validator, &unknown_field, &contract.name, "unknown field");

        let mut next_major = instance.clone();
        next_major["schema_version"] = Value::String("2.0.0".to_owned());
        assert_rejected(
            &validator,
            &next_major,
            &contract.name,
            "next major version",
        );

        let mut unknown_status = instance;
        unknown_status["status"] = Value::String("unknown".to_owned());
        assert_rejected(
            &validator,
            &unknown_status,
            &contract.name,
            "unknown status",
        );
    }
}

#[test]
fn discovered_contract_files_have_exact_fixture_mapping_and_unique_json_keys() {
    assert_eq!(contracts().len(), 4);
    assert!(
        parse_json_without_duplicate_keys(r#"{"schema_version":"1.0.0","schema_version":"1.0.1"}"#)
            .is_err(),
        "duplicate JSON keys must never be silently overwritten"
    );
}

#[test]
fn analysis_manifest_requires_effective_config_and_component_provenance() {
    let validator = validator("analysis-manifest");
    let golden = read_json(repository_root().join("tests/golden/analysis-manifest-v1.json"));

    for required in ["config_hash", "analyzers", "parsers"] {
        let mut missing = golden.clone();
        missing
            .as_object_mut()
            .expect("manifest must be an object")
            .remove(required);
        assert_rejected(
            &validator,
            &missing,
            "analysis-manifest",
            &format!("missing {required}"),
        );
    }

    let analyzers = golden["analyzers"]
        .as_array()
        .expect("golden analyzers must be an array");
    assert!(!analyzers.is_empty(), "at least one analyzer is required");
    let mut sorted_analyzers = analyzers.clone();
    sorted_analyzers.sort_by(|left, right| {
        left["name"]
            .as_str()
            .cmp(&right["name"].as_str())
            .then_with(|| left["version"].as_str().cmp(&right["version"].as_str()))
    });
    assert_eq!(analyzers, &sorted_analyzers, "analyzers must be sorted");

    let mut duplicate_analyzer = golden.clone();
    duplicate_analyzer["analyzers"] =
        serde_json::json!([analyzers[0].clone(), analyzers[0].clone()]);
    assert_rejected(
        &validator,
        &duplicate_analyzer,
        "analysis-manifest",
        "duplicate analyzer component",
    );

    let mut duplicate_parser = golden;
    duplicate_parser["parsers"] = serde_json::json!([
        {"name": "tree-sitter-typescript", "version": "0.23.2"},
        {"name": "tree-sitter-typescript", "version": "0.23.2"}
    ]);
    assert_rejected(
        &validator,
        &duplicate_parser,
        "analysis-manifest",
        "duplicate parser component",
    );
}

#[test]
fn public_revision_contracts_reject_git_null_object_ids() {
    for null_oid in ["0".repeat(40), "0".repeat(64)] {
        for (contract, pointer) in [
            ("analysis-manifest", "/source_snapshot/revision"),
            ("project-evidence", "/provenance/repository_revision"),
            ("project-evaluation", "/project/revision"),
        ] {
            assert_golden_value_rejected(
                contract,
                pointer,
                Value::String(null_oid.clone()),
                "null revision",
            );
        }
    }
}

#[test]
fn rfc3339_utc_fields_require_semantically_valid_dates() {
    for (contract, pointer) in [
        ("analysis-manifest", "/generated_at"),
        ("project-evidence", "/provenance/collected_at"),
    ] {
        assert_golden_value_rejected(
            contract,
            pointer,
            Value::String("2026-13-40T25:61:61Z".to_owned()),
            "malformed date-time",
        );
    }
}

#[test]
fn remote_record_ids_are_portable_non_path_identifiers() {
    for unsafe_id in [
        "/records/1",
        "C:/records/1",
        "records\\1",
        "records/../1",
        "records/./1",
    ] {
        for (contract, pointer) in [
            ("analysis-manifest", "/data_sources/0/remote_record_id"),
            ("project-evidence", "/provenance/remote_record_id"),
        ] {
            assert_golden_value_rejected(
                contract,
                pointer,
                Value::String(unsafe_id.to_owned()),
                "unsafe remote record identifier",
            );
        }
    }
}

#[test]
fn unavailable_project_evidence_is_an_availability_envelope_not_a_fact() {
    let root = repository_root();
    let validator = validator("project-evidence");
    let complete = read_json(root.join("tests/golden/project-evidence-v1.json"));
    let mut unavailable = complete.clone();
    unavailable["grade"] = Value::Null;
    unavailable
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("provenance");
    unavailable
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("payload");
    unavailable["requested_kind"] = Value::String("file".to_owned());
    unavailable["reason"] = Value::String("source_unavailable".to_owned());
    for status in ["unavailable", "unsupported", "insufficient", "pending"] {
        let mut envelope = unavailable.clone();
        envelope["status"] = Value::String(status.to_owned());
        let errors = validation_messages(&validator, &envelope);
        assert!(
            errors.is_empty(),
            "{status} availability envelope must validate: {errors:#?}"
        );
    }
    unavailable["status"] = Value::String("unavailable".to_owned());

    let mut partial = complete.clone();
    partial["status"] = Value::String("partial".to_owned());
    let errors = validation_messages(&validator, &partial);
    assert!(
        errors.is_empty(),
        "partial factual evidence must validate: {errors:#?}"
    );

    let mut contradictory = unavailable.clone();
    contradictory["provenance"] = complete["provenance"].clone();
    contradictory["payload"] = complete["payload"].clone();
    assert_rejected(
        &validator,
        &contradictory,
        "project-evidence",
        "unavailable evidence with factual payload",
    );

    let mut graded_unavailable = unavailable.clone();
    graded_unavailable["grade"] = Value::String("a".to_owned());
    assert_rejected(
        &validator,
        &graded_unavailable,
        "project-evidence",
        "unavailable evidence with grade",
    );

    let mut factless_complete = unavailable.clone();
    factless_complete["status"] = Value::String("complete".to_owned());
    factless_complete["grade"] = Value::String("a".to_owned());
    assert_rejected(
        &validator,
        &factless_complete,
        "project-evidence",
        "complete evidence without factual payload",
    );

    let mut incomplete_fact = complete;
    incomplete_fact
        .as_object_mut()
        .expect("evidence must be an object")
        .remove("provenance");
    assert_rejected(
        &validator,
        &incomplete_fact,
        "project-evidence",
        "complete evidence without immutable provenance",
    );
}

#[test]
fn potential_has_a_distinct_forecast_contract_with_cited_context() {
    let root = repository_root();
    let validator = validator("project-evaluation");
    let evaluation = read_json(root.join("tests/golden/project-evaluation-v1.json"));
    let potential = &evaluation["scores"]["potential"];
    assert!(potential.get("forecast_horizon").is_some());
    assert!(potential.get("assumptions").is_some());
    assert!(potential.get("major_counter_signals").is_some());
    assert!(
        evaluation["scores"]["assay_score"]
            .get("forecast_horizon")
            .is_none(),
        "Assay Score must not absorb Potential forecast fields"
    );

    for required in ["forecast_horizon", "assumptions", "major_counter_signals"] {
        let mut missing = evaluation.clone();
        missing["scores"]["potential"]
            .as_object_mut()
            .expect("potential must be an object")
            .remove(required);
        assert_rejected(
            &validator,
            &missing,
            "project-evaluation",
            &format!("Potential missing {required}"),
        );
    }

    let mut invalid_horizon = evaluation.clone();
    invalid_horizon["scores"]["potential"]["forecast_horizon"] =
        Value::String("twelve_months".to_owned());
    assert_rejected(
        &validator,
        &invalid_horizon,
        "project-evaluation",
        "non-ISO-8601 Potential horizon",
    );

    assert_golden_value_rejected(
        "project-evaluation",
        "/scores/potential/assumptions/0/evidence_ids",
        Value::Array(Vec::new()),
        "uncited Potential assumption",
    );

    let mut generic_score_with_forecast = evaluation;
    generic_score_with_forecast["scores"]["assay_score"]["forecast_horizon"] =
        Value::String("P1Y".to_owned());
    assert_rejected(
        &validator,
        &generic_score_with_forecast,
        "project-evaluation",
        "Assay Score with Potential-only fields",
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
            assert_rejected(&validator, &instance, &contract.name, fixture_name);

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
fn privacy_citation_and_product_domain_boundaries_are_enforced() {
    for unsafe_path in [
        "/private/source.rs",
        "../source.rs",
        "C:\\private\\source.rs",
    ] {
        assert_golden_value_rejected(
            "project-evidence",
            "/payload/relative_path",
            Value::String(unsafe_path.to_owned()),
            "non-portable source path",
        );
    }
    assert_golden_value_rejected(
        "project-evidence",
        "/privacy/visibility",
        Value::String("private_local".to_owned()),
        "private evidence without an explicit transmission boundary",
    );

    assert_golden_value_rejected(
        "ai-judgment",
        "/judgments/0/evidence_ids",
        Value::Array(Vec::new()),
        "uncited applicable judgment",
    );
    assert_golden_value_rejected(
        "ai-judgment",
        "/judgments/0/rating",
        Value::Number(5.into()),
        "out-of-range rating",
    );

    assert_golden_mutation_rejected("project-evaluation", "person-level score", |instance| {
        instance["scores"]
            .as_object_mut()
            .expect("scores must be an object")
            .insert("person_performance".to_owned(), Value::Null);
    });
    assert_golden_mutation_rejected(
        "project-evaluation",
        "numeric score without evidence citations",
        |instance| {
            instance["scores"]["project_substance"]["status"] =
                Value::String("complete".to_owned());
            instance["scores"]["project_substance"]["value"] = Value::Number(80.into());
        },
    );
    assert_golden_value_rejected(
        "analysis-manifest",
        "/status",
        Value::String("complete".to_owned()),
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
    for contract in contracts() {
        let schema = read_json(
            repository_root()
                .join("schemas")
                .join(&contract.name)
                .join("v1.json"),
        );
        assert_closed_objects_and_internal_refs(&schema, &schema, &contract.name);
    }
}

fn assert_closed_objects_and_internal_refs(schema: &Value, value: &Value, location: &str) {
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
                let pointer = reference
                    .strip_prefix('#')
                    .expect("internal references must begin with #");
                assert!(
                    schema.pointer(pointer).is_some(),
                    "dangling internal reference at {location}: {reference}"
                );
            }
            for (key, child) in object {
                assert_closed_objects_and_internal_refs(
                    schema,
                    child,
                    &format!("{location}/{key}"),
                );
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                assert_closed_objects_and_internal_refs(
                    schema,
                    child,
                    &format!("{location}/{index}"),
                );
            }
        }
        _ => {}
    }
}
