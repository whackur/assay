mod cli_contract_common;

use std::fs;

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;
use sha2::{Digest, Sha256};

use cli_contract_common::fixed_command;

#[test]
fn project_analyze_is_repeatable_private_local_and_has_a_reviewed_digest() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let run = || {
        fixed_command()
            .arg("project")
            .arg("analyze")
            .arg(fixture.path())
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
            .expect("analysis subprocess must start")
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stdout.last(), Some(&b'\n'));
    let digest = hex::encode(Sha256::digest(&first.stdout));
    assert_eq!(
        digest,
        include_str!("../../../tests/golden/cli/project-analyze-v1.sha256").trim()
    );

    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schema_version"], "1.0.0");
    assert_eq!(value["manifest"]["status"], "partial");
    assert!(
        value["evidence"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        value["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["privacy"]["visibility"] == "private_local")
    );
    let portable_partial_classifications = value["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| {
            fact["status"] == "partial"
                && fact["payload"]["kind"] == "file_classification"
                && fact["payload"]["reason"] == "attributes_unavailable"
        })
        .map(|fact| fact["id"].clone())
        .collect::<Vec<_>>();
    assert!(!portable_partial_classifications.is_empty());
    let attribute_limitation = value["manifest"]["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|limitation| limitation["code"] == "attribute_resolution_unavailable")
        .expect("portable partial classifications require an attribute limitation");
    assert_eq!(
        attribute_limitation["affected_evidence_ids"],
        Value::Array(portable_partial_classifications)
    );
    let text = String::from_utf8(first.stdout.clone()).unwrap();
    assert!(!text.contains(&fixture.path().display().to_string()));
    assert!(!text.contains("fixture-author@example.invalid"));
    assert!(!text.contains("return left + right"));
    assert!(!text.contains("person"));
    // The public numeric Assay Score stays behind the sufficiency and
    // calibration gates: the field is present but its value is null while
    // essential dimensions remain unscored.
    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        value["evaluation"]["scores"]["assay_score"]["value"],
        Value::Null
    );
    assert_eq!(
        value["evaluation"]["scores"]["assay_score"]["status"],
        "insufficient"
    );
}

#[test]
fn invalid_input_and_missing_sources_use_stable_redacted_channels() {
    let unknown = fixed_command().arg("unknown-command").output().unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    assert!(!unknown.stderr.windows(2).any(|bytes| bytes == b"\x1b["));

    let secret_path = "/missing/private/token-should-not-appear";
    let missing = fixed_command()
        .args([
            "project",
            "analyze",
            secret_path,
            "--output",
            "-",
            "--no-color",
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(4));
    assert!(missing.stdout.is_empty());
    let diagnostic = String::from_utf8(missing.stderr).unwrap();
    assert_eq!(diagnostic, "error: source_not_found\n");
    assert!(!diagnostic.contains(secret_path));
}

#[test]
fn output_file_is_atomic_noclobber_and_stdout_stays_empty() {
    let fixture = RepositoryFixture::build(RepositoryScenario::MissingReadmeAndLicense)
        .expect("fixture must build");
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("result.json");
    let run = || {
        fixed_command()
            .arg("project")
            .arg("analyze")
            .arg(fixture.path())
            .args(["--output"])
            .arg(&destination)
            .args(["--format", "json", "--no-color", "--non-interactive"])
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stdout.is_empty());
    assert!(first.stderr.is_empty());
    let original = fs::read(&destination).unwrap();
    serde_json::from_slice::<Value>(&original).unwrap();

    let second = run();
    assert_eq!(second.status.code(), Some(12));
    assert!(second.stdout.is_empty());
    assert_eq!(fs::read(&destination).unwrap(), original);
    assert_eq!(
        String::from_utf8(second.stderr).unwrap(),
        "error: output_failed code=destination_exists\n"
    );
}

#[test]
fn default_repository_and_revision_are_accepted_in_a_non_tty() {
    let fixture = RepositoryFixture::build(RepositoryScenario::PythonProject).unwrap();
    let output = fixed_command()
        .current_dir(fixture.path())
        .args([
            "project",
            "analyze",
            "--output",
            "-",
            "--format",
            "json",
            "--no-color",
            "--non-interactive",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
}
