use std::{path::PathBuf, str::FromStr};

use assay_domain::{EvidenceId, RepositorySource, RevisionId};
use assay_project_intelligence::{
    ClassificationPolicy, CompilerPolicy, EvaluatorDescriptor, EvaluatorProvider,
    MaturityObservation, MaturitySignal, PotentialContext, ScoreCompilerInput, TypeObservation,
    TypeSignal, Visibility, classify_project,
};
use jsonschema::{Draft, Validator};
use serde_json::Value;

fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate must remain under crates/")
        .to_path_buf()
}

fn evaluation_schema() -> Validator {
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

fn compile_with(classification: assay_project_intelligence::ProjectClassification) -> Value {
    ScoreCompilerInput::new(
        RepositorySource::hosted("github", "example-org", "sample-project").unwrap(),
        RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        EvaluatorDescriptor::new(
            "deterministic-project-evaluator-1",
            EvaluatorProvider::Deterministic,
            None,
            "project-rubric-1",
        )
        .unwrap(),
        Visibility::Public,
        classification,
        Vec::new(),
        None,
        PotentialContext::default(),
        CompilerPolicy::v1(),
    )
    .compile()
    .unwrap()
    .to_machine_value()
}

#[test]
fn a_resolved_classification_validates_against_the_evaluation_schema() {
    let outcome = classify_project(
        &[TypeObservation::new(
            TypeSignal::LibraryPackagingDeclared,
            vec![evidence("evidence:repository:snapshot")],
        )
        .unwrap()],
        &[MaturityObservation::new(
            MaturitySignal::StableReleaseTagged,
            vec![evidence("evidence:repository:snapshot")],
        )
        .unwrap()],
        &ClassificationPolicy::v1(),
    );
    let compiled = compile_with(outcome.classification().clone());
    assert_eq!(
        compiled["classification"]["primary_type"],
        "library_sdk_framework"
    );
    assert_eq!(compiled["classification"]["maturity"], "stable");
    assert_eq!(compiled["classification"]["status"], "complete");
    let errors = evaluation_schema()
        .iter_errors(&compiled)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "classification failed the schema: {errors:#?}"
    );
}

#[test]
fn an_unknown_classification_is_insufficient_with_a_null_type_and_maturity() {
    let outcome = classify_project(&[], &[], &ClassificationPolicy::v1());
    assert_eq!(outcome.primary_type(), None);
    let compiled = compile_with(outcome.classification().clone());
    assert_eq!(compiled["classification"]["status"], "insufficient");
    assert!(compiled["classification"]["primary_type"].is_null());
    assert!(compiled["classification"]["maturity"].is_null());
    let errors = evaluation_schema()
        .iter_errors(&compiled)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "unknown classification failed the schema: {errors:#?}"
    );
}
