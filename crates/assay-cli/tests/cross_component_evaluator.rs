//! Deterministic evaluator schema-validity cross-component test.

mod cross_component;

use cross_component::common;

use assay_test_fixtures::{RepositoryFixture, RepositoryScenario};
use serde_json::Value;

use common::{analyze, assert_valid};

#[test]
fn deterministic_evaluator_produces_schema_valid_evaluation_without_network() {
    // The deterministic evaluator runs locally without network calls and
    // produces a project-evaluation instance embedded in the analysis bundle.
    let fixture = RepositoryFixture::build(RepositoryScenario::TypeScriptProject)
        .expect("fixture must build");
    let output = analyze(fixture.path());
    let bundle: Value = serde_json::from_slice(&output).expect("analysis must be JSON");

    let evaluation = &bundle["evaluation"];
    assert_valid("project-evaluation", evaluation);
    assert_eq!(evaluation["evaluation_version"], "project-intelligence-1");
    assert_eq!(evaluation["evaluator"]["provider"], "deterministic");
    assert_eq!(
        evaluation["evaluator"]["rubric_version"],
        "project-rubric-1"
    );
    assert_eq!(evaluation["visibility"], "private_local");
    // The public numeric Assay Score stays behind the sufficiency gate.
    assert_eq!(evaluation["scores"]["assay_score"]["value"], Value::Null);
    assert_eq!(
        evaluation["scores"]["assay_score"]["status"],
        "insufficient"
    );
    // The compiler recorded the judgment bundle hash binding.
    assert!(evaluation["compiler"]["judgment_bundle_hash"].is_string());
}
