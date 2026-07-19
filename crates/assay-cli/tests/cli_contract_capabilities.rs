mod cli_contract_common;

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use cli_contract_common::fixed_command;

#[test]
fn capabilities_are_exact_schema_valid_and_do_not_claim_future_features() {
    let output = fixed_command()
        .args([
            "capabilities",
            "--format",
            "json",
            "--output",
            "-",
            "--no-color",
        ])
        .output()
        .expect("capabilities subprocess must start");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let golden: Value =
        serde_json::from_str(include_str!("../../../tests/golden/capabilities-v1.json")).unwrap();
    let mut expected = serde_json::to_vec_pretty(&golden).unwrap();
    expected.push(b'\n');
    assert_eq!(output.stdout, expected);
    assert!(!output.stdout.windows(2).any(|bytes| bytes == b"\x1b["));
}

#[test]
fn ai_evaluator_selection_stays_consent_gated_and_deterministic() {
    // ADR 0012: consent gating runs before provider construction. Without a
    // consent grant no external provider is constructed, so selecting a
    // registered AI evaluator ID returns the same deterministic evidence as
    // the default and exits zero.
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let run = |evaluator: &str| {
        fixed_command()
            .arg("project")
            .arg("analyze")
            .arg(fixture.path())
            .args([
                "--revision",
                "HEAD",
                "--evaluator",
                evaluator,
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
    let deterministic = run("deterministic");
    assert!(deterministic.status.success());
    for evaluator in ["openai-api-1", "codex-cli-1"] {
        let gated = run(evaluator);
        assert!(
            gated.status.success(),
            "{evaluator}: {}",
            String::from_utf8_lossy(&gated.stderr)
        );
        assert_eq!(gated.stdout, deterministic.stdout);
    }
    // An unregistered evaluator ID is rejected as invalid input.
    let unknown = run("unregistered-evaluator");
    assert!(!unknown.status.success());
}
