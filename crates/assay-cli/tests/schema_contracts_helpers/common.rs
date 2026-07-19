use std::{fmt, fs, path::PathBuf};

use jsonschema::{Draft, Registry, Resource, Validator};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct Contract {
    pub name: String,
    pub golden: PathBuf,
    pub invalid_fixtures: Vec<PathBuf>,
}

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

pub fn contracts() -> Vec<Contract> {
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

pub fn read_json(path: PathBuf) -> Value {
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    parse_json_without_duplicate_keys(&contents)
        .unwrap_or_else(|error| panic!("invalid JSON in {}: {error}", path.display()))
}

pub fn parse_json_without_duplicate_keys(contents: &str) -> Result<Value, String> {
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

pub fn validator(contract: &str) -> Validator {
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
        let resource = Resource::from_contents(schema);
        (id, resource)
    });
    let registry = Registry::new()
        .extend(resources)
        .expect("public schema URIs must be valid")
        .prepare()
        .expect("public schema registry must build");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .should_validate_formats(true)
        .build(&schema)
        .unwrap_or_else(|error| panic!("invalid {contract} schema: {error}"))
}

pub fn validation_messages(validator: &Validator, instance: &Value) -> Vec<String> {
    validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect()
}

pub fn golden(contract: &str) -> Value {
    read_json(
        repository_root()
            .join("tests/golden")
            .join(format!("{contract}-v1.json")),
    )
}

pub fn refresh_project_artifact(bundle: &mut Value) {
    let evidence = bundle["evidence"].as_array_mut().unwrap();
    evidence.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let bytes = serde_json::to_vec(evidence).unwrap();
    bundle["manifest"]["artifacts"][0]["record_count"] = Value::from(evidence.len());
    bundle["manifest"]["artifacts"][0]["content_hash"] =
        Value::String(format!("sha256:{}", hex::encode(Sha256::digest(bytes))));
}

pub fn assert_rejected(validator: &Validator, instance: &Value, contract: &str, case: &str) {
    assert!(
        !validator.is_valid(instance),
        "{case} unexpectedly satisfied {contract}"
    );
}
