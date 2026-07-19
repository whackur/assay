use std::str::FromStr;

use assay_domain::{
    AnalysisVersion, ContentHash, EvidenceStatus, RubricApplicability, RubricCriterionId,
    RubricJudgment, RubricJudgmentSet,
};
use assay_project_intelligence::{
    CompilerPolicy, PotentialContext, ScoreCompileErrorKind, ScoreCompilerInput, ScoreDimension,
    Visibility,
};

mod score_compiler_helpers;
use score_compiler_helpers::{
    assert_schema_valid, contribution, deterministic_evaluator, golden_classification,
    project_source, revision, snapshot_evidence,
};

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
