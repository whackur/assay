//! Evidence bundle and evaluator fixture builders for cross-component tests.

use std::str::FromStr;

use assay_ai_evaluator::{
    DeterministicFakeProvider, Evaluator, EvidenceBundle, EvidenceDescriptor, EvidenceKind,
    EvidenceScope, ExternalTransmission, QualitativeRubric, ValidatedJudgmentSet,
};
use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId, RubricJudgmentSet};
use assay_project_intelligence::{
    CitedStatement, CompiledEvaluation, CompilerPolicy, EvaluatorDescriptor, EvaluatorProvider,
    PotentialContext, ProjectClassification, ProjectMaturity, ProjectType, ScoreCompilerInput,
    Visibility,
};
use serde_json::Value;

pub(crate) fn evidence_id(bundle: &Value, kind: &str) -> EvidenceId {
    let raw = bundle["evidence"]
        .as_array()
        .expect("evidence must be an array")
        .iter()
        .find(|fact| fact["payload"]["kind"] == kind)
        .unwrap_or_else(|| panic!("fresh evidence must contain a {kind} fact"))["id"]
        .as_str()
        .expect("evidence id must be a string");
    EvidenceId::from_str(raw).expect("CLI evidence id must be a valid domain id")
}

fn descriptor(id: EvidenceId, kind: EvidenceKind, statement: &str) -> EvidenceDescriptor {
    EvidenceDescriptor::new(id, kind, statement).expect("bounded statement must be accepted")
}

// Builds the evaluation bundle the missing CLI adapter would build, keyed by
// real producer evidence identifiers.
pub(crate) fn evidence_bundle(ids: [EvidenceId; 3]) -> EvidenceBundle {
    let [claim, test, fact] = ids;
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::NotUsed,
        vec![
            descriptor(
                claim,
                EvidenceKind::DocumentationClaim,
                "The project documents a repository analysis workflow.",
            ),
            descriptor(
                test,
                EvidenceKind::Test,
                "A cited test exercises the documented workflow.",
            ),
            descriptor(
                fact,
                EvidenceKind::RepositoryFact,
                "The analyzed source revision is immutable.",
            ),
        ],
    )
    .expect("evidence bundle must be valid")
}

pub(crate) fn validated_judgments(bundle: &EvidenceBundle) -> ValidatedJudgmentSet {
    Evaluator::new(QualitativeRubric::project_v1())
        .evaluate(&DeterministicFakeProvider::valid(), bundle)
        .expect("the deterministic provider must produce a valid judgment set")
}

pub(crate) fn compile(judgments: RubricJudgmentSet, primary: EvidenceId) -> CompiledEvaluation {
    let classification = ProjectClassification::new(
        EvidenceStatus::Complete,
        Some(ProjectType::CliDeveloperTool),
        vec![ProjectType::LibrarySdkFramework],
        vec!["developer_tool".to_owned()],
        Some(ProjectMaturity::Prototype),
        0.76,
        vec![primary.clone()],
    )
    .expect("classification must be valid");
    let potential = PotentialContext::new(
        vec![
            CitedStatement::new(
                "Continued iteration is required before a numeric forecast.",
                vec![primary.clone()],
            )
            .expect("assumption must cite evidence"),
        ],
        vec![
            CitedStatement::new(
                "The current evidence is insufficient for a numeric forecast.",
                vec![primary.clone()],
            )
            .expect("counter-signal must cite evidence"),
        ],
    );
    ScoreCompilerInput::new(
        RepositorySource::hosted("github", "example-org", "sample-project")
            .expect("project source must be valid"),
        RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").expect("revision"),
        EvaluatorDescriptor::new(
            "deterministic-project-evaluator-1",
            EvaluatorProvider::Deterministic,
            None,
            "project-rubric-1",
        )
        .expect("evaluator descriptor must be valid"),
        Visibility::PrivateLocal,
        classification,
        Vec::new(),
        Some(judgments),
        potential,
        CompilerPolicy::v1(),
    )
    .compile()
    .expect("score compiler must accept the mapped judgments")
}
