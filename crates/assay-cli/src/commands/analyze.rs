use std::{ffi::OsStr, path::Path};

use assay_classifier::{BuiltInPolicy, LinguistAttributeFacts};
use assay_git::{GitCliAdapter, RepositorySnapshotPort, SnapshotRequest};
use assay_local::{ConsentState, GithubTokenEnvVar};
use assay_project_intelligence::{
    ClassifiedSnapshotFile, assemble_project_evidence, build_project_analysis,
    validate_project_bundle_consistency,
};
use serde_json::Value;

use crate::cli::{AnalyzeArgs, Evaluator};
use crate::errors::{
    RunError, analysis_failed, bundle_error, collection_error, collection_or_not_found,
    executable_missing, history_record_invalid, history_write_error, invalid_github_token_env,
    source_not_found,
};
use crate::git::{collection_limits, trusted_git};
use crate::output::json_bytes;
use crate::schema::validate;
use crate::time::current_time;

use super::{Outcome, emit};

pub(crate) fn analyze(arguments: AnalyzeArgs) -> Result<Outcome, RunError> {
    let _delivery_contract = (
        arguments.format,
        arguments.no_color,
        arguments.non_interactive,
    );
    // Consent gating runs before provider construction (ADR 0012). The local
    // slice exposes no consent-granting surface yet, so no matching
    // `ConsentGrant` can exist, no external provider is ever constructed, and
    // deterministic evidence is returned for every evaluator selection; the
    // recorded report keeps its `ai_evaluation` section `disabled` with
    // `user_consent_required`.
    let consent = evaluation_consent(arguments.evaluator);
    // Validate the token variable *name* eagerly; the value is never read here.
    // An already-cloned local repository is analyzed without credentials.
    if let Some(name) = &arguments.github_token_env {
        GithubTokenEnvVar::parse(name).map_err(|_| invalid_github_token_env())?;
    }
    let git = trusted_git().ok_or_else(executable_missing)?;
    if !arguments.repository.exists() {
        return Err(source_not_found());
    }
    let adapter = GitCliAdapter::from_trusted_executable(git, collection_limits()?)
        .map_err(collection_error)?;
    let identity = adapter
        .derive_local_repository_source(&arguments.repository, OsStr::new(&arguments.revision))
        .map_err(collection_or_not_found)?;
    let snapshot = adapter
        .collect(SnapshotRequest::new(
            &arguments.repository,
            identity.source().clone(),
            OsStr::new(identity.revision().as_str()),
        ))
        .map_err(collection_or_not_found)?;
    let classifications = snapshot
        .entries()
        .iter()
        .map(|entry| {
            ClassifiedSnapshotFile::classify(
                &snapshot,
                entry,
                LinguistAttributeFacts::unavailable(),
                &BuiltInPolicy::V1,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| analysis_failed("file_classification"))?;
    let evidence = assemble_project_evidence(&snapshot, classifications)
        .map_err(|_| analysis_failed("evidence_assembly"))?;
    let generated_at = current_time()?;
    let value = build_project_analysis(&snapshot, &evidence, &generated_at)
        .map_err(|_| analysis_failed("machine_mapping"))?;
    validate("project-analysis", &value)?;
    validate_project_bundle_consistency(&value).map_err(|_| bundle_error())?;
    if let Some(directory) = &arguments.record_history {
        record_local_history(directory, value.clone(), &consent, &generated_at)?;
    }
    Ok(emit(json_bytes(&value)?, arguments.output))
}

/// Returns the consent posture governing one analysis run. Every selectable
/// evaluator ID starts from the no-grant default: `deterministic` performs no
/// AI evaluation, and the AI evaluator IDs (see
/// [`crate::evaluators::EVALUATOR_REGISTRY`]) require an explicit informed grant
/// that no local surface can produce yet, so they stay consent-gated.
fn evaluation_consent(evaluator: Evaluator) -> ConsentState {
    let _selected = evaluator.id();
    ConsentState::default()
}

fn record_local_history(
    directory: &Path,
    analysis: Value,
    consent: &ConsentState,
    generated_at: &str,
) -> Result<(), RunError> {
    let report = assay_local::LocalReport::from_analysis(analysis, consent, generated_at)
        .map_err(|_| history_record_invalid())?;
    let store =
        assay_local::LocalHistoryStore::open(directory).map_err(|_| history_write_error())?;
    store
        .append(report.to_value(), generated_at)
        .map_err(|_| history_write_error())?;
    Ok(())
}
