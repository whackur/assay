//! Serve/history round-trip and consent-gating cross-component tests.

mod cross_component;

use cross_component::common;
use cross_component::serve;

use std::process::Command;

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use common::FIXED_TIME;
use serve::serve_once_get;

#[test]
fn record_history_round_trips_the_local_report_contract_over_serve() {
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let history = tempfile::TempDir::new().unwrap();

    let recorded = Command::new(common::binary())
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", FIXED_TIME)
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args(["--evaluator", "deterministic", "--output", "-"])
        .arg("--record-history")
        .arg(history.path())
        .output()
        .expect("analyze subprocess must start");
    assert!(recorded.status.success());

    let response = serve_once_get(history.path(), "/api/history/rec-000001");
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "response: {response}"
    );
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .expect("response must have a body");
    let report: Value = serde_json::from_str(body.trim()).expect("served report must be JSON");

    assert_eq!(report["schema_version"], "1.0.0");
    assert_eq!(report["visibility"], "private_local");
    assert_eq!(report["privacy"]["visibility"], "private_local");
    assert_eq!(report["privacy"]["catalog_eligible"], false);
    assert_eq!(
        report["privacy"]["external_transmission"],
        "consent_required"
    );
    assert_eq!(report["sections"]["ai_evaluation"]["state"], "disabled");
    assert_eq!(
        report["sections"]["competitor_discovery"]["state"],
        "disabled"
    );
    // The immutable analysis the CLI produced is embedded verbatim.
    assert_eq!(report["analysis"]["schema_version"], "1.0.0");
}

#[test]
fn private_repository_ai_processing_requires_explicit_consent() {
    // ADR 0012: the local slice exposes no consent-granting surface, so the
    // recorded report keeps its `ai_evaluation` section `disabled` with
    // `user_consent_required`. The deterministic evaluator still runs because
    // it performs no external transmission, but no external provider is ever
    // constructed for a private repository without an explicit grant.
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let history = tempfile::TempDir::new().unwrap();

    let recorded = Command::new(common::binary())
        .env_clear()
        .env("ASSAY_TEST_FIXED_TIME", FIXED_TIME)
        .arg("project")
        .arg("analyze")
        .arg(fixture.path())
        .args(["--evaluator", "openai-api-1", "--output", "-"])
        .arg("--record-history")
        .arg(history.path())
        .output()
        .expect("analyze subprocess must start");
    assert!(recorded.status.success());

    let response = serve_once_get(history.path(), "/api/history/rec-000001");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .expect("response must have a body");
    let report: Value = serde_json::from_str(body.trim()).expect("served report must be JSON");

    // The AI evaluation section stays disabled pending explicit consent.
    assert_eq!(report["sections"]["ai_evaluation"]["state"], "disabled");
    assert_eq!(
        report["sections"]["ai_evaluation"]["reason"],
        "user_consent_required"
    );
    assert_eq!(
        report["privacy"]["external_transmission"],
        "consent_required"
    );
    // The deterministic evaluation still ran and is embedded in the analysis.
    assert_eq!(report["analysis"]["schema_version"], "1.0.0");
    assert_eq!(
        report["analysis"]["evaluation"]["evaluation_version"],
        "project-intelligence-1"
    );
}
