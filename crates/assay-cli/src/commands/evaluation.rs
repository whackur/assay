//! WIRE-001: wires the manifest-to-bundle adapter, the deterministic
//! evaluator, and the score compiler into `assay project analyze`.
//!
//! The chain is deterministic and network-free by default. Private-source AI
//! processing requires explicit consent; without it the evaluation section
//! stays `disabled` with `user_consent_required` and no external provider is
//! constructed. The public numeric Assay Score remains behind the
//! sufficiency and calibration gates in the compiler.

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
///
/// Returns the compiled evaluation machine value on success. The evaluation is
/// deterministic and performs no network I/O. Private-source evidence stays
/// `PrivateLocal` with `NotUsed` external transmission, so no consent grant is
/// required for the deterministic path; external providers remain consent-gated
/// and are not constructed here.
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

/// Returns the consent posture governing one analysis run. The local slice
/// exposes no consent-granting surface yet, so every selectable evaluator ID
/// starts from the no-grant default: the deterministic evaluator runs without
/// external transmission, and the AI evaluator IDs require an explicit informed
/// grant that no local surface can produce yet, so they stay consent-gated.
pub(crate) fn evaluation_consent(_evaluator_id: &str) -> ConsentState {
    ConsentState::default()
}

/// Returns true when the deterministic evaluator may run for this consent
/// posture. The deterministic evaluator performs no external transmission, so
/// it runs without consent. External providers require an explicit grant.
pub(crate) fn deterministic_evaluation_allowed(_consent: &ConsentState) -> bool {
    true
}

/// Builds a classification input for the score compiler from the manifest.
///
/// The deterministic classifier does not yet resolve a project type or maturity
/// from the manifest, so the classification is unavailable and the compiler
/// keeps the score unscored rather than inventing one. A future deterministic
/// rule will populate this without a provider.
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
