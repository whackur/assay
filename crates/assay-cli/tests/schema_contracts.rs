use std::{fmt, fs, path::PathBuf};

use assay_project_intelligence::validate_project_bundle_consistency;
use jsonschema::{Draft, Resource, Validator};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

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
    let root = repository_root();
    let resources = contracts().into_iter().map(|candidate| {
        let schema = read_json(root.join("schemas").join(&candidate.name).join("v1.json"));
        let id = schema["$id"]
            .as_str()
            .expect("every public schema must declare an ID")
            .to_owned();
        let resource = Resource::from_contents(schema)
            .expect("a bundled public schema must be a valid resource");
        (id, resource)
    });
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_resources(resources)
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

fn refresh_project_artifact(bundle: &mut Value) {
    let evidence = bundle["evidence"].as_array_mut().unwrap();
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let bytes = serde_json::to_vec(evidence).unwrap();
    bundle["manifest"]["artifacts"][0]["record_count"] = Value::from(evidence.len());
    bundle["manifest"]["artifacts"][0]["content_hash"] =
        Value::String(format!("sha256:{:x}", Sha256::digest(bytes)));
}

fn tracked_file_record() -> Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "repository": {
            "kind": "local",
            "repository_id": format!("sha256:{}", "1".repeat(64))
        },
        "id": "evidence:tracked-file:v1-golden",
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": "0123456789abcdef0123456789abcdef01234567",
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "remote_record_id": null
        },
        "payload": {
            "kind": "tracked_file",
            "path": { "encoding": "utf8", "value": "src/main.ts" },
            "mode": "regular",
            "object_kind": "blob",
            "object_id": "89abcdef0123456789abcdef0123456789abcdef",
            "content_status": "complete",
            "language": "TypeScript",
            "language_status": "complete",
            "size_bytes": 418,
            "content_hash": format!("sha256:{}", "4".repeat(64)),
            "issue": null
        }
    })
}

fn classification_record() -> Value {
    serde_json::json!({
        "schema_version": "1.0.0",
        "repository": {
            "kind": "local",
            "repository_id": format!("sha256:{}", "1".repeat(64))
        },
        "id": "evidence:file-classification:v1-golden",
        "status": "complete",
        "grade": "a",
        "privacy": {
            "visibility": "public",
            "source_content": "not_retained",
            "external_transmission": "not_requested"
        },
        "related_evidence_ids": ["evidence:tracked-file:v1-golden"],
        "attempted_policy_version": "classifier-v1",
        "provenance": {
            "source_kind": "repository_content",
            "collected_at": "2026-01-02T03:04:06Z",
            "repository_revision": "0123456789abcdef0123456789abcdef01234567",
            "content_hash": null,
            "remote_record_id": null
        },
        "payload": {
            "kind": "file_classification",
            "source_evidence_id": "evidence:tracked-file:v1-golden",
            "policy_version": "classifier-v1",
            "reason": null,
            "classification": {
                "primary_category": "production_code",
                "tags": [],
                "rule_id": "path.production.typescript",
                "confidence": 1.0,
                "evidence": [{
                    "kind": "policy_rule",
                    "rule_id": "path.production.typescript",
                    "attribute_name": null,
                    "attribute_value": null
                }]
            }
        }
    })
}

fn repository_feature_record(status: &str, state: &str, related: Value) -> Value {
    let mut feature = tracked_file_record();
    feature["id"] = Value::String("evidence:repository-feature:v1-golden".into());
    feature["status"] = Value::String(status.into());
    feature["provenance"]["content_hash"] = Value::Null;
    feature["payload"] = serde_json::json!({
        "kind": "repository_feature",
        "feature": "readme",
        "state": state,
        "related_evidence_ids": related
    });
    feature
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

fn assert_golden_value_valid(contract: &str, pointer: &str, value: Value, case: &str) {
    let validator = validator(contract);
    let mut instance = golden(contract);
    *instance
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("missing golden pointer {contract}{pointer}")) = value;
    let errors = validation_messages(&validator, &instance);
    assert!(errors.is_empty(), "{case} failed {contract}: {errors:#?}");
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
fn repository_feature_schema_closes_state_status_and_citation_cardinality() {
    let validator = validator("project-evidence");
    for (case, status, state, related) in [
        (
            "present without supporting evidence",
            "complete",
            "present",
            serde_json::json!([]),
        ),
        (
            "unavailable without cause evidence",
            "partial",
            "unavailable",
            serde_json::json!([]),
        ),
        (
            "absent with contradictory evidence",
            "complete",
            "absent",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "partial present feature",
            "partial",
            "present",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "complete unavailable feature",
            "complete",
            "unavailable",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
    ] {
        assert_rejected(
            &validator,
            &repository_feature_record(status, state, related),
            "project-evidence",
            case,
        );
    }

    for (case, status, state, related) in [
        (
            "cited present feature",
            "complete",
            "present",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "cited unavailable feature",
            "partial",
            "unavailable",
            serde_json::json!(["evidence:tracked-file:v1-golden"]),
        ),
        (
            "uncited absent feature",
            "complete",
            "absent",
            serde_json::json!([]),
        ),
    ] {
        let feature = repository_feature_record(status, state, related);
        let errors = validation_messages(&validator, &feature);
        assert!(
            errors.is_empty(),
            "{case} failed project-evidence: {errors:#?}"
        );
    }
}

#[test]
fn project_analysis_bundle_cross_component_invariants_are_enforced() {
    let bundle = golden("project-analysis");
    validate_project_bundle_consistency(&bundle).expect("reviewed bundle must be coherent");

    let mut foreign_source = bundle.clone();
    foreign_source["evidence"][0]["repository"]["repository_id"] =
        Value::String(format!("sha256:{}", "9".repeat(64)));
    assert!(validate_project_bundle_consistency(&foreign_source).is_err());

    let mut wrong_revision = bundle.clone();
    wrong_revision["evidence"][0]["provenance"]["repository_revision"] =
        Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
    assert!(validate_project_bundle_consistency(&wrong_revision).is_err());

    let mut wrong_count = bundle.clone();
    wrong_count["manifest"]["artifacts"][0]["record_count"] = Value::from(3);
    assert!(validate_project_bundle_consistency(&wrong_count).is_err());

    let mut wrong_role = bundle.clone();
    wrong_role["manifest"]["artifacts"][0]["role"] = Value::String("other".into());
    assert!(validate_project_bundle_consistency(&wrong_role).is_err());

    let mut wrong_status = bundle.clone();
    wrong_status["manifest"]["artifacts"][0]["status"] = Value::String("partial".into());
    assert!(validate_project_bundle_consistency(&wrong_status).is_err());

    let mut wrong_source_revision = bundle.clone();
    wrong_source_revision["manifest"]["data_sources"][0]["revision"] =
        Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
    assert!(validate_project_bundle_consistency(&wrong_source_revision).is_err());

    let mut duplicate = bundle;
    let repeated = duplicate["evidence"][0].clone();
    duplicate["evidence"].as_array_mut().unwrap().push(repeated);
    assert!(validate_project_bundle_consistency(&duplicate).is_err());
}

#[test]
fn project_analysis_bundle_references_are_closed_over_the_evidence_set() {
    let bundle = golden("project-analysis");

    for (case, mutate) in [
        (
            "data source",
            (|value: &mut Value| {
                value["manifest"]["data_sources"][0]["id"] =
                    Value::String("evidence:missing:data-source".into());
            }) as fn(&mut Value),
        ),
        ("warning", |value: &mut Value| {
            value["manifest"]["warnings"] = serde_json::json!([{
                "code": "test_warning",
                "affected_evidence_ids": ["evidence:missing:warning"]
            }]);
        }),
        ("limitation", |value: &mut Value| {
            value["manifest"]["limitations"][0]["affected_evidence_ids"][0] =
                Value::String("evidence:missing:limitation".into());
        }),
        ("top-level relation", |value: &mut Value| {
            value["evidence"][0]["related_evidence_ids"] =
                serde_json::json!(["evidence:missing:top-level"]);
            refresh_project_artifact(value);
        }),
    ] {
        let mut invalid = bundle.clone();
        mutate(&mut invalid);
        assert!(
            validate_project_bundle_consistency(&invalid).is_err(),
            "unknown {case} reference must be rejected"
        );
    }

    let mut payload_relation = bundle;
    let mut feature = tracked_file_record();
    feature["id"] = Value::String("evidence:repository-feature:v1-golden".into());
    feature["provenance"]["content_hash"] = Value::Null;
    feature["payload"] = serde_json::json!({
        "kind": "repository_feature",
        "feature": "readme",
        "state": "present",
        "related_evidence_ids": ["evidence:missing:payload"]
    });
    payload_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .push(feature);
    refresh_project_artifact(&mut payload_relation);
    assert!(
        validate_project_bundle_consistency(&payload_relation).is_err(),
        "unknown payload relation must be rejected"
    );
}

#[test]
fn project_analysis_bundle_classification_citations_and_policy_are_consistent() {
    let mut bundle = golden("project-analysis");
    bundle["evidence"]
        .as_array_mut()
        .unwrap()
        .extend([tracked_file_record(), classification_record()]);
    refresh_project_artifact(&mut bundle);
    validate_project_bundle_consistency(&bundle).expect("coherent classification must validate");

    let mut wrong_relation = bundle.clone();
    let classification = wrong_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["related_evidence_ids"] =
        serde_json::json!(["evidence:history-scope:v1-golden"]);
    refresh_project_artifact(&mut wrong_relation);
    assert!(validate_project_bundle_consistency(&wrong_relation).is_err());

    let mut extra_relation = bundle.clone();
    let classification = extra_relation["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["related_evidence_ids"] = serde_json::json!([
        "evidence:history-scope:v1-golden",
        "evidence:tracked-file:v1-golden"
    ]);
    refresh_project_artifact(&mut extra_relation);
    assert!(validate_project_bundle_consistency(&extra_relation).is_err());

    let mut wrong_policy = bundle;
    let classification = wrong_policy["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "file_classification")
        .unwrap();
    classification["attempted_policy_version"] = Value::String("classifier-v2".into());
    refresh_project_artifact(&mut wrong_policy);
    assert!(validate_project_bundle_consistency(&wrong_policy).is_err());
}

#[test]
fn project_analysis_bundle_snapshot_and_history_facts_match_the_manifest() {
    let bundle = golden("project-analysis");
    for (case, mutate) in [
        (
            "snapshot root tree",
            (|value: &mut Value| {
                let snapshot = value["evidence"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|fact| fact["payload"]["kind"] == "repository_snapshot")
                    .unwrap();
                snapshot["payload"]["root_tree"] =
                    Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
                refresh_project_artifact(value);
            }) as fn(&mut Value),
        ),
        ("snapshot commit time", |value: &mut Value| {
            let snapshot = value["evidence"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|fact| fact["payload"]["kind"] == "repository_snapshot")
                .unwrap();
            snapshot["payload"]["commit_time"] = Value::String("2026-01-02T03:04:04Z".into());
            refresh_project_artifact(value);
        }),
        ("history count", |value: &mut Value| {
            value["manifest"]["scope"]["commit_count"] = Value::from(4);
        }),
        ("history head", |value: &mut Value| {
            value["manifest"]["scope"]["head_revision"] =
                Value::String("abcdef0123456789abcdef0123456789abcdef01".into());
        }),
        ("history status", |value: &mut Value| {
            value["manifest"]["scope"]["history_status"] = Value::String("partial".into());
        }),
        ("repository data-source status", |value: &mut Value| {
            value["manifest"]["data_sources"][0]["status"] = Value::String("partial".into());
        }),
        ("history data-source status", |value: &mut Value| {
            value["manifest"]["data_sources"][1]["status"] = Value::String("partial".into());
        }),
    ] {
        let mut invalid = bundle.clone();
        mutate(&mut invalid);
        assert!(
            validate_project_bundle_consistency(&invalid).is_err(),
            "mismatched {case} must be rejected"
        );
    }
}

#[test]
fn project_analysis_bundle_content_hash_and_status_redundancy_is_closed() {
    let mut tracked = golden("project-analysis");
    tracked["evidence"]
        .as_array_mut()
        .unwrap()
        .push(tracked_file_record());
    refresh_project_artifact(&mut tracked);
    validate_project_bundle_consistency(&tracked).expect("coherent tracked file must validate");

    let mut wrong_hash = tracked;
    let fact = wrong_hash["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact["payload"]["kind"] == "tracked_file")
        .unwrap();
    fact["provenance"]["content_hash"] = Value::String(format!("sha256:{}", "5".repeat(64)));
    refresh_project_artifact(&mut wrong_hash);
    assert!(validate_project_bundle_consistency(&wrong_hash).is_err());

    let mut complete_with_partial_evidence = golden("project-analysis");
    complete_with_partial_evidence["evidence"][0]["status"] = Value::String("partial".into());
    refresh_project_artifact(&mut complete_with_partial_evidence);
    assert!(validate_project_bundle_consistency(&complete_with_partial_evidence).is_err());

    let mut unjustified_partial = golden("project-analysis");
    unjustified_partial["manifest"]["status"] = Value::String("partial".into());
    unjustified_partial["manifest"]["artifacts"][0]["status"] = Value::String("partial".into());
    assert!(validate_project_bundle_consistency(&unjustified_partial).is_err());
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
    assert_eq!(contracts().len(), 6);
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
fn new_raw_and_derived_file_contracts_preserve_partial_state() {
    let validator = validator("project-evidence");
    let mut tracked = golden("project-evidence");
    tracked["status"] = Value::String("partial".into());
    tracked["payload"] = serde_json::json!({
        "kind": "tracked_file",
        "path": { "encoding": "utf8", "value": "src/main.ts" },
        "mode": "regular",
        "object_kind": "blob",
        "object_id": "89abcdef0123456789abcdef0123456789abcdef",
        "content_status": "partial",
        "language": "TypeScript",
        "language_status": "complete",
        "size_bytes": 20_000_000,
        "content_hash": null,
        "issue": "size_limit"
    });
    assert!(validator.is_valid(&tracked));

    let mut fake_complete = tracked.clone();
    fake_complete["payload"]["content_status"] = Value::String("complete".into());
    assert_rejected(
        &validator,
        &fake_complete,
        "project-evidence",
        "complete content without hash",
    );

    let mut fake_unavailable = tracked.clone();
    fake_unavailable["payload"]["content_status"] = Value::String("unavailable".into());
    fake_unavailable["payload"]["size_bytes"] = Value::from(0);
    fake_unavailable["payload"]["content_hash"] =
        Value::String(format!("sha256:{}", "0".repeat(64)));
    fake_unavailable["payload"]["issue"] = Value::String("timeout".into());
    assert_rejected(
        &validator,
        &fake_unavailable,
        "project-evidence",
        "unavailable content with fabricated values",
    );

    let mut gitlink = tracked.clone();
    gitlink["payload"] = serde_json::json!({
        "kind": "tracked_file",
        "path": { "encoding": "git_path_hex", "value": "7375626d6f64756c65" },
        "mode": "gitlink",
        "object_kind": "commit",
        "object_id": "89abcdef0123456789abcdef0123456789abcdef",
        "content_status": "unsupported",
        "language": null,
        "language_status": "unsupported",
        "size_bytes": null,
        "content_hash": null,
        "issue": "gitlink_content"
    });
    assert!(validator.is_valid(&gitlink));
    gitlink["payload"]["object_kind"] = Value::String("blob".into());
    assert_rejected(
        &validator,
        &gitlink,
        "project-evidence",
        "gitlink blob contradiction",
    );

    let mut classification = golden("project-evidence");
    classification["status"] = Value::String("partial".into());
    classification["related_evidence_ids"] = serde_json::json!(["evidence:file:src-main-ts"]);
    classification["attempted_policy_version"] = Value::String("classifier-v1".into());
    classification["payload"] = serde_json::json!({
        "kind": "file_classification",
        "source_evidence_id": "evidence:file:src-main-ts",
        "policy_version": "classifier-v1",
        "reason": "attributes_unavailable",
        "classification": {
            "primary_category": "production_code",
            "tags": ["production"],
            "rule_id": "path.production.typescript",
            "confidence": 1.0,
            "evidence": [{
                "kind": "attribute_facts_unavailable",
                "rule_id": "attributes.unavailable",
                "attribute_name": null,
                "attribute_value": null
            }]
        }
    });
    assert!(validator.is_valid(&classification));
    classification["payload"]["reason"] = Value::Null;
    assert_rejected(
        &validator,
        &classification,
        "project-evidence",
        "partial classification without reason",
    );
}

#[test]
fn tracked_language_presence_matches_its_supported_status() {
    let validator = validator("project-evidence");
    let mut tracked = golden("project-evidence");
    tracked["payload"] = tracked_file_record()["payload"].clone();
    assert!(validator.is_valid(&tracked));

    let mut named_but_unsupported = tracked.clone();
    named_but_unsupported["payload"]["language_status"] = Value::String("unsupported".into());
    assert_rejected(
        &validator,
        &named_but_unsupported,
        "project-evidence",
        "named language with unsupported status",
    );

    let mut unnamed_but_complete = tracked;
    unnamed_but_complete["payload"]["language"] = Value::Null;
    assert_rejected(
        &validator,
        &unnamed_but_complete,
        "project-evidence",
        "null language with complete status",
    );
}

#[test]
fn parent_delta_status_issue_and_observed_counts_are_bidirectional() {
    let validator = validator("project-evidence");
    let mut complete = golden("project-evidence");
    complete["payload"] = serde_json::json!({
        "kind": "parent_delta",
        "changed_entries": 3,
        "renames": 1,
        "issue": null
    });
    assert!(validator.is_valid(&complete));

    let mut partial = complete.clone();
    partial["status"] = Value::String("partial".into());
    partial["payload"]["renames"] = Value::Null;
    partial["payload"]["issue"] = Value::String("rename_candidate_limit".into());
    assert!(validator.is_valid(&partial));

    let mut complete_with_issue = partial.clone();
    complete_with_issue["status"] = Value::String("complete".into());
    assert_rejected(
        &validator,
        &complete_with_issue,
        "project-evidence",
        "complete parent delta with issue",
    );

    let mut partial_without_issue = complete;
    partial_without_issue["status"] = Value::String("partial".into());
    assert_rejected(
        &validator,
        &partial_without_issue,
        "project-evidence",
        "partial parent delta without issue",
    );

    let mut fabricated_rename_count = partial;
    fabricated_rename_count["payload"]["renames"] = Value::from(0);
    assert_rejected(
        &validator,
        &fabricated_rename_count,
        "project-evidence",
        "rename-limit parent delta with fabricated rename count",
    );

    let mut process_failure_payload = golden("project-evidence");
    process_failure_payload["status"] = Value::String("partial".into());
    process_failure_payload["payload"] = serde_json::json!({
        "kind": "parent_delta",
        "changed_entries": null,
        "renames": null,
        "issue": "process_failure"
    });
    assert_rejected(
        &validator,
        &process_failure_payload,
        "project-evidence",
        "process failure represented as a factual parent delta",
    );

    let mut process_failure_envelope = golden("project-evidence");
    process_failure_envelope["status"] = Value::String("unavailable".into());
    process_failure_envelope["grade"] = Value::Null;
    process_failure_envelope
        .as_object_mut()
        .unwrap()
        .remove("provenance");
    process_failure_envelope
        .as_object_mut()
        .unwrap()
        .remove("payload");
    process_failure_envelope["requested_kind"] = Value::String("parent_delta".into());
    process_failure_envelope["reason"] = Value::String("process_failure".into());
    assert!(validator.is_valid(&process_failure_envelope));
}

#[test]
fn classification_availability_evidence_and_envelope_policy_are_consistent() {
    let validator = validator("project-evidence");
    let mut partial = classification_record();
    partial["status"] = Value::String("partial".into());
    partial["payload"]["reason"] = Value::String("attributes_unavailable".into());
    partial["payload"]["classification"]["evidence"] = serde_json::json!([
        {
            "kind": "policy_rule",
            "rule_id": "path.production.typescript",
            "attribute_name": null,
            "attribute_value": null
        },
        {
            "kind": "attribute_facts_unavailable",
            "rule_id": "attributes.unavailable",
            "attribute_name": null,
            "attribute_value": null
        }
    ]);
    assert!(validator.is_valid(&partial));

    let mut partial_without_unavailable_fact = partial.clone();
    partial_without_unavailable_fact["payload"]["classification"]["evidence"] = serde_json::json!([{
        "kind": "policy_rule",
        "rule_id": "path.production.typescript",
        "attribute_name": null,
        "attribute_value": null
    }]);
    assert_rejected(
        &validator,
        &partial_without_unavailable_fact,
        "project-evidence",
        "partial classification without unavailable attribute evidence",
    );

    let mut complete_with_unavailable_fact = partial;
    complete_with_unavailable_fact["status"] = Value::String("complete".into());
    complete_with_unavailable_fact["payload"]["reason"] = Value::Null;
    assert_rejected(
        &validator,
        &complete_with_unavailable_fact,
        "project-evidence",
        "complete classification with unavailable attribute evidence",
    );

    let mut missing = golden("project-evidence");
    missing["status"] = Value::String("unavailable".into());
    missing["grade"] = Value::Null;
    missing.as_object_mut().unwrap().remove("provenance");
    missing.as_object_mut().unwrap().remove("payload");
    missing["requested_kind"] = Value::String("file_classification".into());
    missing["reason"] = Value::String("missing_classification".into());
    missing["related_evidence_ids"] = serde_json::json!(["evidence:tracked-file:v1-golden"]);
    assert!(
        validator.is_valid(&missing),
        "missing classification must not invent an attempted policy"
    );

    let mut missing_with_policy = missing.clone();
    missing_with_policy["attempted_policy_version"] = Value::String("classifier-v1".into());
    assert_rejected(
        &validator,
        &missing_with_policy,
        "project-evidence",
        "missing classification with invented policy",
    );

    let mut empty_relation = missing;
    empty_relation["related_evidence_ids"] = serde_json::json!([]);
    assert_rejected(
        &validator,
        &empty_relation,
        "project-evidence",
        "classification envelope without a source citation",
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
fn potential_forecast_horizon_is_a_positive_iso_8601_duration() {
    const POINTER: &str = "/scores/potential/forecast_horizon";

    for zero_duration in [
        "P0D",
        "PT0S",
        "P0Y0M0DT0H0M0S",
        "PT0.0S",
        "P0Y0M0DT0H0M0.000S",
    ] {
        assert_golden_value_rejected(
            "project-evaluation",
            POINTER,
            Value::String(zero_duration.to_owned()),
            "zero Potential forecast horizon",
        );
    }

    for positive_duration in ["P1D", "P1Y", "PT1H"] {
        assert_golden_value_valid(
            "project-evaluation",
            POINTER,
            Value::String(positive_duration.to_owned()),
            "positive Potential forecast horizon",
        );
    }

    for invalid_duration in ["PT0.5S", "P1Dgarbage", "not-a-duration-1", "P1DT"] {
        assert_golden_value_rejected(
            "project-evaluation",
            POINTER,
            Value::String(invalid_duration.to_owned()),
            "invalid Potential forecast horizon containing a non-zero digit",
        );
    }
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
                "capabilities-v1-future-claim.json" => {
                    instance["commands"] = serde_json::json!(["capabilities", "project analyze"]);
                }
                "project-analysis-v1-invalid-nested-manifest.json" => {
                    instance = golden("project-analysis");
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
fn public_schemas_are_closed_and_use_only_internal_or_bundled_references() {
    for contract in contracts() {
        let schema = read_json(
            repository_root()
                .join("schemas")
                .join(&contract.name)
                .join("v1.json"),
        );
        assert_closed_objects_and_bundled_refs(&schema, &schema, &contract.name);
    }
}

fn assert_closed_objects_and_bundled_refs(schema: &Value, value: &Value, location: &str) {
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
                if let Some(pointer) = reference.strip_prefix('#') {
                    assert!(
                        pointer.starts_with("/$defs/") && schema.pointer(pointer).is_some(),
                        "dangling or non-canonical internal reference at {location}: {reference}"
                    );
                } else {
                    assert!(
                        matches!(
                            reference,
                            "https://schemas.assay.dev/analysis-manifest/v1.json"
                                | "https://schemas.assay.dev/project-evidence/v1.json"
                        ) && location.starts_with("project-analysis/"),
                        "unregistered or non-composition reference at {location}: {reference}"
                    );
                }
            }
            for (key, child) in object {
                assert_closed_objects_and_bundled_refs(schema, child, &format!("{location}/{key}"));
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                assert_closed_objects_and_bundled_refs(
                    schema,
                    child,
                    &format!("{location}/{index}"),
                );
            }
        }
        _ => {}
    }
}
