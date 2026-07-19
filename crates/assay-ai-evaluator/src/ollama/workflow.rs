use assay_project_intelligence::{
    HostedEvaluationAttempt, HostedEvaluationInput, HostedEvaluationPort, HostedFailure,
};
use serde_json::json;

use crate::{EvaluationSnapshot, SecretStore, api::SnapshotOutcome};

use super::config::{
    OLLAMA_COMPATIBLE_PROFILE, OLLAMA_COMPATIBLE_PROVIDER_ID, OllamaCompatibleConfig,
};
use super::disposition::classify_ollama_failure;
use super::evaluator::OllamaCompatibleEvaluator;
use super::transport::OllamaCompatibleHttpTransport;

/// Workflow-facing evaluator adapter. Provider configuration and secrets stay
/// inside this crate; the application entrypoint only wires the port.
pub struct HostedOllamaWorkflowEvaluator<S> {
    config: Option<OllamaCompatibleConfig>,
    secret_store: S,
}

impl<S> HostedOllamaWorkflowEvaluator<S> {
    pub const fn new(config: Option<OllamaCompatibleConfig>, secret_store: S) -> Self {
        Self {
            config,
            secret_store,
        }
    }
}

impl<S> HostedEvaluationPort for HostedOllamaWorkflowEvaluator<S>
where
    S: SecretStore + Clone + Send + Sync + 'static,
{
    async fn evaluate(
        &self,
        input: &HostedEvaluationInput,
    ) -> Result<HostedEvaluationAttempt, HostedFailure> {
        let Some(config) = self.config.clone() else {
            return Ok(unavailable_attempt("unconfigured", "ollama_unconfigured"));
        };
        let facts = input.normalized_facts.clone();
        let secret_store = self.secret_store.clone();
        tokio::task::spawn_blocking(move || {
            let transport = OllamaCompatibleHttpTransport::new()
                .map_err(|_| HostedFailure::provider("ollama_transport_unavailable", true))?;
            let evaluator = OllamaCompatibleEvaluator::new(config, secret_store, transport);
            let snapshot = evaluator
                .evaluate_hosted_metadata(&facts)
                .map_err(|error| {
                    let disposition = classify_ollama_failure(error.kind());
                    HostedFailure::provider(disposition.code(), disposition.retryable())
                })?;
            match snapshot.outcome() {
                SnapshotOutcome::Validated(judgment) if judgment.is_usable() => {
                    let judgment = serde_json::to_value(judgment)
                        .expect("validated judgments must be JSON serializable");
                    Ok(workflow_attempt(
                        &snapshot,
                        "validated_unpublished",
                        None,
                        Some(judgment),
                    ))
                }
                SnapshotOutcome::Validated(_) => {
                    Ok(workflow_attempt(&snapshot, "unavailable", None, None))
                }
                SnapshotOutcome::Failed(kind) => {
                    let disposition = classify_ollama_failure(*kind);
                    if disposition.retryable() {
                        let mut failure = HostedFailure::provider(disposition.code(), true);
                        failure.retry_after_seconds = snapshot
                            .telemetry()
                            .and_then(|telemetry| telemetry.retry_after())
                            .and_then(|delay| i64::try_from(delay.as_secs()).ok());
                        failure.evaluation_attempt = Some(Box::new(workflow_attempt(
                            &snapshot,
                            "partial",
                            Some(disposition.code()),
                            None,
                        )));
                        Err(failure)
                    } else {
                        Ok(workflow_attempt(
                            &snapshot,
                            "unavailable",
                            Some(disposition.code()),
                            None,
                        ))
                    }
                }
            }
        })
        .await
        .map_err(|_| HostedFailure::provider("ollama_transport_unavailable", true))?
    }
}

fn unavailable_attempt(model: &str, code: &str) -> HostedEvaluationAttempt {
    HostedEvaluationAttempt {
        provider_id: OLLAMA_COMPATIBLE_PROVIDER_ID.to_owned(),
        model: model.to_owned(),
        evaluator_profile: OLLAMA_COMPATIBLE_PROFILE.to_owned(),
        rubric_version: "project-rubric-1".to_owned(),
        prompt_version: "not_attempted".to_owned(),
        evaluation_version: "not_attempted".to_owned(),
        provider_profile_version: OLLAMA_COMPATIBLE_PROFILE.to_owned(),
        sampling: json!({}),
        evidence_bundle_hash: "not_attempted".to_owned(),
        usage: None,
        latency_ms: None,
        status: "unavailable".to_owned(),
        error_code: Some(code.to_owned()),
        judgment: None,
    }
}

fn workflow_attempt(
    snapshot: &EvaluationSnapshot,
    status: &str,
    error_code: Option<&str>,
    judgment: Option<serde_json::Value>,
) -> HostedEvaluationAttempt {
    let provenance = snapshot.provenance();
    let sampling = provenance.sampling();
    let telemetry = snapshot.telemetry();
    HostedEvaluationAttempt {
        provider_id: provenance.provider_id().to_owned(),
        model: provenance.model().to_owned(),
        evaluator_profile: OLLAMA_COMPATIBLE_PROFILE.to_owned(),
        rubric_version: provenance.rubric_version().to_owned(),
        prompt_version: provenance.prompt_version().to_owned(),
        evaluation_version: provenance.evaluation_version().to_owned(),
        provider_profile_version: OLLAMA_COMPATIBLE_PROFILE.to_owned(),
        sampling: json!({
            "temperature": sampling.temperature,
            "top_p": sampling.top_p,
            "max_output_tokens": sampling.max_output_tokens,
            "seed": sampling.seed,
        }),
        evidence_bundle_hash: provenance.evidence_bundle_hash().to_owned(),
        usage: telemetry.and_then(|value| value.usage()).map(|usage| {
            json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            })
        }),
        latency_ms: telemetry.and_then(|value| i64::try_from(value.latency().as_millis()).ok()),
        status: status.to_owned(),
        error_code: error_code.map(str::to_owned),
        judgment,
    }
}
