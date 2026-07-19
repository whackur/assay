mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{
    assert_rejected, contracts, read_json, validation_messages, validator,
};

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
