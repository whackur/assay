use std::{path::PathBuf, str::FromStr};

use assay_domain::{
    AnalysisVersion, ContentHash, EvidenceId, EvidenceStatus, RepositorySource, RevisionId,
    RubricApplicability, RubricCriterionId, RubricJudgment, RubricJudgmentSet,
};
use assay_project_intelligence::{
    CitedStatement, CompilerPolicy, DeterministicContribution, EvaluatorDescriptor,
    EvaluatorProvider, PotentialContext, ProjectClassification, ProjectMaturity, ProjectType,
    ScoreCompileErrorKind, ScoreCompilerInput, ScoreDimension, Visibility,
};
use jsonschema::{Draft, Validator};
use serde_json::Value;

fn revision() -> RevisionId {
    RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap()
}

fn project_source() -> RepositorySource {
    RepositorySource::hosted("github", "example-org", "sample-project").unwrap()
}

fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

fn snapshot_evidence() -> EvidenceId {
    evidence("evidence:repository:snapshot")
}

fn deterministic_evaluator() -> EvaluatorDescriptor {
    EvaluatorDescriptor::new(
        "deterministic-project-evaluator-1",
        EvaluatorProvider::Deterministic,
        None,
        "project-rubric-1",
    )
    .unwrap()
}

fn golden_classification() -> ProjectClassification {
    ProjectClassification::new(
        EvidenceStatus::Complete,
        Some(ProjectType::CliDeveloperTool),
        vec![ProjectType::LibrarySdkFramework],
        vec!["developer_tool".to_owned()],
        Some(ProjectMaturity::Prototype),
        0.76,
        vec![snapshot_evidence()],
    )
    .unwrap()
}

fn golden_potential_context() -> PotentialContext {
    PotentialContext::new(
        vec![
            CitedStatement::new(
                "Future potential depends on evidence of continued project iteration.",
                vec![snapshot_evidence()],
            )
            .unwrap(),
        ],
        vec![
            CitedStatement::new(
                "The current evidence is insufficient for a numeric forecast.",
                vec![snapshot_evidence()],
            )
            .unwrap(),
        ],
    )
}

fn golden_input() -> ScoreCompilerInput {
    ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::PrivateLocal,
        golden_classification(),
        Vec::new(),
        None,
        golden_potential_context(),
        CompilerPolicy::v1(),
    )
}

fn contribution(
    rule_id: &str,
    dimension: ScoreDimension,
    value: f64,
    confidence: f64,
) -> DeterministicContribution {
    DeterministicContribution::new(
        rule_id,
        dimension,
        RubricApplicability::Applicable,
        Some(value),
        confidence,
        vec![snapshot_evidence()],
    )
    .unwrap()
}

fn essential_contributions() -> Vec<DeterministicContribution> {
    vec![
        contribution(
            "substance.working_implementation",
            ScoreDimension::ProjectSubstance,
            0.8,
            0.6,
        ),
        contribution(
            "engineering_rigor.tests_present",
            ScoreDimension::EngineeringRigor,
            0.6,
            0.6,
        ),
        contribution(
            "open_source_readiness.license_present",
            ScoreDimension::OpenSourceReadiness,
            1.0,
            0.6,
        ),
    ]
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

fn assert_schema_valid(value: &Value) {
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
        DeterministicContribution::new(
            "originality.differentiation",
            ScoreDimension::Originality,
            RubricApplicability::NotApplicable,
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
fn validated_rubric_judgments_contribute_bounded_ratings_only() {
    let bundle_hash = ContentHash::from_str(&format!("sha256:{}", "a".repeat(64))).unwrap();
    let judgment = RubricJudgment::new(
        RubricCriterionId::from_str("substance.claim_implementation_fit").unwrap(),
        RubricApplicability::Applicable,
        Some(4),
        4,
        0.8,
        vec![snapshot_evidence()],
    )
    .unwrap();
    let set = RubricJudgmentSet::new(
        AnalysisVersion::from_str("project-intelligence-1").unwrap(),
        AnalysisVersion::from_str("project-rubric-1").unwrap(),
        EvidenceStatus::Partial,
        bundle_hash,
        vec![judgment],
    )
    .unwrap();
    let input = ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::Public,
        golden_classification(),
        vec![
            contribution(
                "engineering_rigor.tests_present",
                ScoreDimension::EngineeringRigor,
                0.6,
                0.6,
            ),
            contribution(
                "open_source_readiness.license_present",
                ScoreDimension::OpenSourceReadiness,
                1.0,
                0.6,
            ),
        ],
        Some(set),
        PotentialContext::default(),
        CompilerPolicy::v1(),
    );
    let compiled = input.compile().unwrap();
    // The maximal rating (4/4) yields a substance value of 100 from the judgment.
    let substance = compiled
        .dimension(ScoreDimension::ProjectSubstance)
        .unwrap();
    assert_eq!(substance.value(), Some(100.0));
    // The judgment bundle hash is recorded; the provider never emits a score.
    let value = compiled.to_machine_value();
    assert_eq!(
        value["compiler"]["judgment_bundle_hash"].as_str(),
        Some(format!("sha256:{}", "a".repeat(64)).as_str())
    );
    assert_schema_valid(&value);
}

#[test]
fn a_rubric_version_mismatch_is_rejected() {
    let bundle_hash = ContentHash::from_str(&format!("sha256:{}", "c".repeat(64))).unwrap();
    let judgment = RubricJudgment::new(
        RubricCriterionId::from_str("substance.claim_implementation_fit").unwrap(),
        RubricApplicability::Applicable,
        Some(3),
        4,
        0.8,
        vec![snapshot_evidence()],
    )
    .unwrap();
    let set = RubricJudgmentSet::new(
        AnalysisVersion::from_str("project-intelligence-1").unwrap(),
        AnalysisVersion::from_str("project-rubric-2").unwrap(),
        EvidenceStatus::Partial,
        bundle_hash,
        vec![judgment],
    )
    .unwrap();
    let input = ScoreCompilerInput::new(
        project_source(),
        revision(),
        deterministic_evaluator(),
        Visibility::Public,
        golden_classification(),
        Vec::new(),
        Some(set),
        PotentialContext::default(),
        CompilerPolicy::v1(),
    );
    assert_eq!(
        input.compile().unwrap_err().kind(),
        ScoreCompileErrorKind::RubricVersionMismatch
    );
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
