//! WIRE-001: wires manifest-to-bundle adapter, deterministic evaluator, and score compiler into `assay project analyze`.
//! Network-free by default; private-source AI requires explicit consent, else the evaluation section stays `disabled`.

use assay_ai_evaluator::{
    AdapterPrivacy, DeterministicFakeProvider, Evaluator as AiEvaluator, QualitativeRubric,
    manifest_to_evidence_bundle,
};
use assay_domain::{EvidenceStatus, RepositorySource, RevisionId};
use assay_local::ConsentState;
use assay_project_intelligence::{
    CompilerPolicy, EvaluatorDescriptor, EvaluatorProvider, ProjectClassification,
    ProjectEvidenceManifest, ScoreCompilerInput, Visibility,
};

use crate::errors::{RunError, analysis_failed};

/// Runs the deterministic evaluator and score compiler over one manifest.
/// Deterministic, no network I/O; external providers stay consent-gated.
pub(crate) fn compile_deterministic_evaluation(
    manifest: &ProjectEvidenceManifest,
    classification: &ProjectClassification,
    project_source: RepositorySource,
    revision: RevisionId,
) -> Result<serde_json::Value, RunError> {
    let privacy = AdapterPrivacy::local_deterministic();
    let bundle = manifest_to_evidence_bundle(manifest, privacy)
        .map_err(|_| analysis_failed("evidence_bundle"))?;
    let evaluator = AiEvaluator::new(QualitativeRubric::project_v1());
    let validated = evaluator
        .evaluate(&DeterministicFakeProvider::valid(), &bundle)
        .map_err(|_| analysis_failed("ai_evaluation"))?;
    let judgment_set = validated
        .to_rubric_judgment_set()
        .map_err(|_| analysis_failed("judgment_mapping"))?;
    let evaluator_descriptor = EvaluatorDescriptor::new(
        "deterministic-project-evaluator-1",
        EvaluatorProvider::Deterministic,
        None,
        "project-rubric-1",
    )
    .map_err(|_| analysis_failed("evaluator_descriptor"))?;
    let potential_context = assay_project_intelligence::PotentialContext::default();
    let input = ScoreCompilerInput::new(
        project_source,
        revision,
        evaluator_descriptor,
        Visibility::PrivateLocal,
        classification.clone(),
        Vec::new(),
        Some(judgment_set),
        potential_context,
        CompilerPolicy::v1(),
    );
    let compiled = input
        .compile()
        .map_err(|_| analysis_failed("score_compilation"))?;
    Ok(compiled.to_machine_value())
}

/// Consent posture for one analysis run. No local consent-granting surface yet, so AI evaluator IDs stay consent-gated.
pub(crate) fn evaluation_consent(_evaluator_id: &str) -> ConsentState {
    ConsentState::default()
}

/// True when the deterministic evaluator may run; it performs no external transmission, so no consent required.
pub(crate) fn deterministic_evaluation_allowed(_consent: &ConsentState) -> bool {
    true
}

/// Builds a classification input for the score compiler. Classification is unavailable until a future deterministic rule resolves it.
pub(crate) fn classification_for_compilation(
    manifest: &ProjectEvidenceManifest,
) -> Result<ProjectClassification, RunError> {
    let evidence_ids = manifest.all_evidence_ids().cloned().collect::<Vec<_>>();
    ProjectClassification::new(
        EvidenceStatus::Unavailable,
        None,
        Vec::new(),
        Vec::new(),
        None,
        0.0,
        evidence_ids,
    )
    .map_err(|_| analysis_failed("classification_for_compilation"))
}
