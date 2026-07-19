use serde_json::Value;

use super::common::{assert_rejected, golden, validation_messages, validator};

pub fn assert_golden_mutation_rejected(
    contract: &str,
    case: &str,
    mutate: impl FnOnce(&mut Value),
) {
    let validator = validator(contract);
    let mut instance = golden(contract);
    mutate(&mut instance);
    assert_rejected(&validator, &instance, contract, case);
}

pub fn assert_golden_value_rejected(contract: &str, pointer: &str, value: Value, case: &str) {
    assert_golden_mutation_rejected(contract, case, |instance| {
        *instance
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("missing golden pointer {contract}{pointer}")) = value;
    });
}

pub fn assert_golden_value_valid(contract: &str, pointer: &str, value: Value, case: &str) {
    let validator = validator(contract);
    let mut instance = golden(contract);
    *instance
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("missing golden pointer {contract}{pointer}")) = value;
    let errors = validation_messages(&validator, &instance);
    assert!(errors.is_empty(), "{case} failed {contract}: {errors:#?}");
}

pub fn assert_closed_objects_and_bundled_refs(schema: &Value, value: &Value, location: &str) {
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
                                | "https://schemas.assay.dev/project-evaluation/v1.json"
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
