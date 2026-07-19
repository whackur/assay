use jsonschema::{Draft, Registry, Resource, Validator};
use serde_json::Value;

use crate::errors::{RunError, schema_configuration_failed, schema_validation_failed};

pub(crate) fn validate(contract: &str, value: &Value) -> Result<(), RunError> {
    let validator = schema_validator(contract).map_err(|_| schema_configuration_failed())?;
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(schema_validation_failed())
    }
}

fn schema_validator(contract: &str) -> Result<Validator, ()> {
    let schemas = [
        (
            "analysis-manifest",
            include_str!("../../../schemas/analysis-manifest/v1.json"),
        ),
        (
            "capabilities",
            include_str!("../../../schemas/capabilities/v1.json"),
        ),
        (
            "project-analysis",
            include_str!("../../../schemas/project-analysis/v1.json"),
        ),
        (
            "project-evidence",
            include_str!("../../../schemas/project-evidence/v1.json"),
        ),
    ];
    let mut parsed = BTreeSchemas::default();
    for (name, text) in schemas {
        parsed.insert(name, serde_json::from_str(text).map_err(|_| ())?);
    }
    let resources = parsed
        .values()
        .map(|schema| {
            let id = schema["$id"].as_str().ok_or(())?.to_owned();
            let resource = Resource::from_contents(schema.clone());
            Ok((id, resource))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let registry = Registry::new()
        .extend(resources)
        .map_err(|_| ())?
        .prepare()
        .map_err(|_| ())?;
    let root = parsed.get(contract).ok_or(())?;
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .with_registry(&registry)
        .build(root)
        .map_err(|_| ())
}

type BTreeSchemas = std::collections::BTreeMap<&'static str, Value>;
