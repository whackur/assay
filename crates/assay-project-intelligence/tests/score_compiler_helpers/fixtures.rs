use std::str::FromStr;

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId, RubricApplicability};
use assay_project_intelligence::{
    CitedStatement, CompilerPolicy, DeterministicContribution, EvaluatorDescriptor,
    EvaluatorProvider, PotentialContext, ProjectClassification, ProjectMaturity, ProjectType,
    ScoreCompilerInput, ScoreDimension, Visibility,
};

pub fn revision() -> RevisionId {
    RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap()
}

pub fn project_source() -> RepositorySource {
    RepositorySource::hosted("github", "example-org", "sample-project").unwrap()
}

pub fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

pub fn snapshot_evidence() -> EvidenceId {
    evidence("evidence:repository:snapshot")
}

pub fn deterministic_evaluator() -> EvaluatorDescriptor {
    EvaluatorDescriptor::new(
        "deterministic-project-evaluator-1",
        EvaluatorProvider::Deterministic,
        None,
        "project-rubric-1",
    )
    .unwrap()
}

pub fn golden_classification() -> ProjectClassification {
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

pub fn golden_potential_context() -> PotentialContext {
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

pub fn golden_input() -> ScoreCompilerInput {
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

pub fn contribution(
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

pub fn essential_contributions() -> Vec<DeterministicContribution> {
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
