use serde_json::Value;

use crate::{
    EvaluationError, EvaluationSnapshot, EvidenceBundle, QualitativeRubric, SecretStore,
    api::ApiKeyEvaluator,
};

use super::config::{OllamaCompatibleConfig, OllamaProfile};
use super::metadata::build_hosted_metadata_bundle;

/// Shared evaluator plus Ollama-specific immutable profile.
pub struct OllamaCompatibleEvaluator<S, T> {
    inner: ApiKeyEvaluator<OllamaProfile, S, T>,
}

impl<S: SecretStore, T: crate::api::HttpTransport> OllamaCompatibleEvaluator<S, T> {
    pub fn new(config: OllamaCompatibleConfig, secret_store: S, transport: T) -> Self {
        Self {
            inner: ApiKeyEvaluator::new(
                QualitativeRubric::project_v1(),
                OllamaProfile::new(config),
                secret_store,
                transport,
            ),
        }
    }

    pub const fn transport(&self) -> &T {
        self.inner.transport()
    }

    pub fn evaluate(&self, bundle: &EvidenceBundle) -> EvaluationSnapshot {
        self.inner.evaluate(bundle)
    }

    pub fn evaluate_hosted_metadata(
        &self,
        facts: &Value,
    ) -> Result<EvaluationSnapshot, EvaluationError> {
        build_hosted_metadata_bundle(facts).map(|bundle| self.evaluate(&bundle))
    }
}
