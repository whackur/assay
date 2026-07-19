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

pub fn run_state_schema() -> Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join("schemas/run-state/v1.json"))
            .expect("run-state schema must be readable"),
    )
    .expect("run-state schema must parse");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .expect("run-state schema must build")
}
