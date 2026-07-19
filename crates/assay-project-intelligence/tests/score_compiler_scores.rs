use assay_domain::EvidenceStatus;
use assay_project_intelligence::{
    CompilerPolicy, PotentialContext, ScoreCompilerInput, ScoreDimension, Visibility,
};
use serde_json::Value;

mod score_compiler_helpers;
use score_compiler_helpers::{
    assert_schema_valid, contribution, deterministic_evaluator, essential_contributions,
    golden_classification, golden_input, project_source, revision,
};

#[test]
fn compiler_reproduces_the_reviewed_insufficient_golden() {
    let compiled = golden_input().compile().unwrap();
    let produced = compiled.to_machine_value();
    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/project-evaluation-v1.json"
    ))
    .expect("reviewed evaluation golden must parse");
    assert_eq!(
        produced, golden,
        "the compiler must reproduce the reviewed evaluation golden"
    );
    assert_schema_valid(&produced);
}

#[test]
fn compilation_is_byte_deterministic() {
    let first = serde_json::to_vec(&golden_input().compile().unwrap().to_machine_value()).unwrap();
    let second = serde_json::to_vec(&golden_input().compile().unwrap().to_machine_value()).unwrap();
    assert_eq!(
        first, second,
        "identical input must yield byte-identical output"
    );
}

#[test]
fn missing_essential_evidence_keeps_the_overall_score_unscored_not_zero() {
    let compiled = golden_input().compile().unwrap();
    let assay = compiled.assay_score();
    assert_eq!(assay.status(), EvidenceStatus::Insufficient);
    assert_eq!(assay.value(), None);
    assert!(!assay.provisional());
    for dimension in [
        ScoreDimension::ProjectSubstance,
        ScoreDimension::EngineeringRigor,
        ScoreDimension::OpenSourceReadiness,
    ] {
        let score = compiled.dimension(dimension).unwrap();
        assert_eq!(score.status(), EvidenceStatus::Insufficient);
        assert_eq!(
            score.value(),
            None,
            "insufficient dimension must not be a zero"
        );
    }
    for dimension in [
        ScoreDimension::Originality,
        ScoreDimension::MaintenanceHealth,
    ] {
        assert_eq!(
            compiled.dimension(dimension).unwrap().status(),
            EvidenceStatus::Unavailable
        );
    }
}

#[test]
fn a_young_project_with_sufficient_essential_evidence_is_provisional() {
    let input = ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::PrivateLocal,
        golden_classification(),
        essential_contributions(),
        None,
        PotentialContext::default(),
        CompilerPolicy::v1(),
    );
    let compiled = input.compile().unwrap();
    let assay = compiled.assay_score();
    assert!(
        assay.provisional(),
        "a young project score must be provisional"
    );
    assert_eq!(assay.status(), EvidenceStatus::Partial);
    let value = assay.value().expect("a provisional score has a value");
    assert!((0.0..=100.0).contains(&value));
    assert!(
        assay.confidence() < 0.6,
        "provisional confidence must be reduced: {}",
        assay.confidence()
    );
    // Non-essential dimensions with no evidence stay unavailable, not zero.
    assert_eq!(
        compiled
            .dimension(ScoreDimension::Originality)
            .unwrap()
            .value(),
        None
    );
    assert_schema_valid(&compiled.to_machine_value());
}

#[test]
fn all_dimensions_present_produces_an_available_non_provisional_score() {
    let mut contributions = essential_contributions();
    contributions.push(contribution(
        "originality.differentiation",
        ScoreDimension::Originality,
        0.7,
        0.6,
    ));
    contributions.push(contribution(
        "maintenance_health.recent_release",
        ScoreDimension::MaintenanceHealth,
        0.5,
        0.6,
    ));
    let input = ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::Public,
        golden_classification(),
        contributions,
        None,
        PotentialContext::default(),
        CompilerPolicy::v1(),
    );
    let compiled = input.compile().unwrap();
    assert!(!compiled.provisional());
    assert_eq!(compiled.assay_score().status(), EvidenceStatus::Complete);
    // Weighted mean of 0.8,0.7,0.6,1.0,0.5 with weights 25,20,25,15,15 -> 71.5.
    let value = compiled.assay_score().value().unwrap();
    assert!(
        (value - 71.5).abs() < 1e-9,
        "unexpected weighted mean: {value}"
    );
    assert_schema_valid(&compiled.to_machine_value());
}

#[test]
fn not_applicable_criteria_are_excluded_rather_than_scored_as_zero() {
    let mut contributions = essential_contributions();
    contributions.push(
        assay_project_intelligence::DeterministicContribution::new(
            "originality.differentiation",
            ScoreDimension::Originality,
            assay_domain::RubricApplicability::NotApplicable,
            None,
            0.0,
            Vec::new(),
        )
        .unwrap(),
    );
    let input = ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::Public,
        golden_classification(),
        contributions,
        None,
        PotentialContext::default(),
        CompilerPolicy::v1(),
    );
    let compiled = input.compile().unwrap();
    let originality = compiled.dimension(ScoreDimension::Originality).unwrap();
    assert_eq!(
        originality.value(),
        None,
        "not_applicable must not become zero"
    );
    assert_eq!(originality.status(), EvidenceStatus::Unavailable);
    // The overall score treats originality as absent, so it stays provisional.
    assert!(compiled.provisional());
}

#[test]
fn potential_is_separate_and_never_folds_into_the_assay_score() {
    let value = golden_input().compile().unwrap().to_machine_value();
    assert!(
        value["scores"]["assay_score"]
            .get("forecast_horizon")
            .is_none(),
        "Assay Score must not absorb Potential forecast fields"
    );
    assert_eq!(
        value["scores"]["potential"]["forecast_horizon"].as_str(),
        Some("P1Y")
    );
    assert_eq!(
        value["scores"]["potential"]["version"].as_str(),
        Some("potential-1")
    );
}
