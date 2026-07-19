use std::path::PathBuf;

use jsonschema::{Draft, Validator};
use serde_json::Value;

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate must remain under crates/")
        .to_path_buf()
}

pub fn evaluation_schema() -> Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join("schemas/project-evaluation/v1.json"))
            .expect("evaluation schema must be readable"),
    )
    .expect("evaluation schema must parse");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .expect("evaluation schema must build")
}

pub fn assert_schema_valid(value: &Value) {
    let validator = evaluation_schema();
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "compiled output failed the schema: {errors:#?}"
    );
}
