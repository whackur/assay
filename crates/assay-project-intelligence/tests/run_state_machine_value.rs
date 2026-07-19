use assay_project_intelligence::Administrator;
use serde_json::Value;

mod run_state_helpers;
use run_state_helpers::{representative_run, run_state_schema};

#[test]
fn machine_value_reproduces_the_reviewed_golden_and_validates() {
    let produced = representative_run().to_machine_value();
    let golden: Value =
        serde_json::from_str(include_str!("../../../tests/golden/run-state-v1.json"))
            .expect("reviewed run-state golden must parse");
    assert_eq!(
        produced, golden,
        "the run model must reproduce the reviewed run-state golden"
    );
    let validator = run_state_schema();
    let errors: Vec<String> = validator
        .iter_errors(&produced)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "golden failed run-state schema: {errors:#?}"
    );
}

#[test]
fn a_purged_run_machine_value_validates_against_the_schema() {
    let admin = Administrator::assume();
    let mut run = representative_run();
    run.purge(&admin, "2026-07-16T09:30:00Z").unwrap();

    let value = run.to_machine_value();
    assert_eq!(value["lifecycle"], "purged");
    // A purged run keeps completed and partial stage statuses for audit, yet its
    // result content is removed, so every result_snapshot is null.
    for stage in value["stages"].as_array().unwrap() {
        assert_eq!(stage["result_snapshot"], Value::Null);
    }
    let validator = run_state_schema();
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "purged run failed run-state schema: {errors:#?}"
    );
}

#[test]
fn machine_value_is_byte_deterministic() {
    let first = serde_json::to_vec(&representative_run().to_machine_value()).unwrap();
    let second = serde_json::to_vec(&representative_run().to_machine_value()).unwrap();
    assert_eq!(first, second);
}
