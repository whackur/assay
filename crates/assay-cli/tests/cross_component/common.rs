//! Shared helpers for the cross-component integration tests.

use std::path::PathBuf;
use std::process::Command;

pub(crate) const FIXED_TIME: &str = "2026-01-02T03:04:06Z";
pub(crate) const PRODUCED_EVALUATION: &str = "tests/integration/produced/project-evaluation.json";

pub(crate) fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_assay")
}

pub(crate) fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("assay-cli must remain under crates/")
        .to_path_buf()
}

// Runs `assay project analyze` on a fixture with a fixed clock and returns the
// machine output written to stdout.
pub(crate) fn analyze(repository: &std::path::Path) -> Vec<u8> {
    let output = Command::new(binary())
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", FIXED_TIME)
        .arg("project")
        .arg("analyze")
        .arg(repository)
        .args([
            "--revision",
            "HEAD",
            "--evaluator",
            "deterministic",
            "--format",
            "json",
            "--output",
            "-",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .expect("analyze subprocess must start");
    assert!(
        output.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "analyze must not log to stderr");
    output.stdout
}

pub(crate) fn schema_validator(contract: &str) -> jsonschema::Validator {
    use jsonschema::{Draft, Registry, Resource};
    use serde_json::Value;

    let schemas = repository_root().join("schemas");
    let mut root = None;
    let resources = std::fs::read_dir(&schemas)
        .expect("schema directory must be readable")
        .filter_map(|entry| {
            let path = entry.expect("schema entry must be readable").path();
            let file = path.join("v1.json");
            if !file.is_file() {
                return None;
            }
            let schema: Value = serde_json::from_str(
                &std::fs::read_to_string(&file).expect("schema must be readable"),
            )
            .expect("schema must parse");
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("schema directory name")
                .to_owned();
            if name == contract {
                root = Some(schema.clone());
            }
            let id = schema["$id"]
                .as_str()
                .expect("schema must declare $id")
                .to_owned();
            Some((id, Resource::from_contents(schema)))
        })
        .collect::<Vec<_>>();
    let root = root.unwrap_or_else(|| panic!("unknown schema contract: {contract}"));
    let registry = Registry::new()
        .extend(resources)
        .expect("schema resource URIs must be valid")
        .prepare()
        .expect("schema registry must build");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .with_registry(&registry)
        .build(&root)
        .expect("schema must build")
}

pub(crate) fn assert_valid(contract: &str, instance: &serde_json::Value) {
    let validator = schema_validator(contract);
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "{contract} rejected fresh producer output: {errors:#?}"
    );
}
