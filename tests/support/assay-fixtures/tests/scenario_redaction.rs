//! Redaction and error-path tests for the deterministic fixture builder.

use assay_test_fixtures::{RepositoryFixture, RepositoryFixtureBuilder, RepositoryScenario};

#[test]
fn fixture_debug_output_redacts_paths_and_source_content() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("the deterministic repository fixture must build");
    let debug = format!("{fixture:?}");
    let machine_path = fixture.path().to_string_lossy();

    assert!(!debug.contains(machine_path.as_ref()));
    assert!(!debug.contains("export function add"));
    assert!(!debug.contains("example.invalid"));
    assert!(debug.contains("<temporary-repository>"));
}

#[test]
fn fixture_build_errors_redact_program_paths_credentials_and_source_content() {
    let sensitive_program = "/machine/private/token/export function add";
    let builder = RepositoryFixtureBuilder::new(RepositoryScenario::TypeScriptProject)
        .git_program(sensitive_program)
        .command_environment("SENSITIVE_FIXTURE_TOKEN", "credential-value");
    let builder_debug = format!("{builder:?}");
    assert!(!builder_debug.contains(sensitive_program));
    assert!(!builder_debug.contains("credential-value"));

    let error = builder
        .build()
        .expect_err("a missing Git executable must fail safely");
    let diagnostics = format!("{error:?} {error}");
    assert!(!diagnostics.contains(sensitive_program));
    assert!(!diagnostics.contains("example.invalid"));
    assert!(!diagnostics.contains("credential-value"));
    assert!(!diagnostics.contains("export function add"));
    assert!(!diagnostics.contains("diff --git"));
}
