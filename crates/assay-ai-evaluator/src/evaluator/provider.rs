use serde_json::{Value, json};

use crate::{AI_JUDGMENT_SCHEMA_VERSION, ProviderError, QualitativeCriterion, TransmissionSurface};

use super::request::ProviderRequest;
use super::types::ProviderExecutionBoundary;

/// Adapter boundary shared by deterministic and future external providers.
pub trait EvaluationProvider {
    /// Returns a stable adapter identifier for provenance outside this result.
    fn provider_id(&self) -> &'static str;

    /// Declares whether evidence stays local or crosses a provider boundary.
    fn execution_boundary(&self) -> ProviderExecutionBoundary;

    /// Declares the widest content surface this provider can transmit: the
    /// bounded bundle alone, or the whole analyzed worktree snapshot.
    fn transmission_surface(&self) -> TransmissionSurface;

    /// Returns untrusted structured bytes. The evaluator validates all fields.
    fn evaluate(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError>;
}

enum FakeResponse {
    Valid,
    Raw(Vec<u8>),
}

/// Network-free deterministic provider for contract and compiler integration tests.
pub struct DeterministicFakeProvider {
    response: FakeResponse,
}

impl DeterministicFakeProvider {
    /// Produces one stable cited judgment for every requested criterion.
    pub const fn valid() -> Self {
        Self {
            response: FakeResponse::Valid,
        }
    }

    /// Returns fixed untrusted bytes for negative contract tests.
    pub fn from_raw_response(response: Vec<u8>) -> Self {
        Self {
            response: FakeResponse::Raw(response),
        }
    }
}

impl std::fmt::Debug for DeterministicFakeProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeterministicFakeProvider")
            .field("response", &"<untrusted-provider-output>")
            .finish()
    }
}

impl EvaluationProvider for DeterministicFakeProvider {
    fn provider_id(&self) -> &'static str {
        "deterministic-fake-1"
    }

    fn execution_boundary(&self) -> ProviderExecutionBoundary {
        ProviderExecutionBoundary::Local
    }

    fn transmission_surface(&self) -> TransmissionSurface {
        TransmissionSurface::BundleOnly
    }

    fn evaluate(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, ProviderError> {
        match &self.response {
            FakeResponse::Raw(response) => Ok(response.clone()),
            FakeResponse::Valid => {
                let evidence_ids = request
                    .evidence()
                    .iter()
                    .map(|item| item.id().as_str())
                    .collect::<Vec<_>>();
                let judgments = request
                    .criteria()
                    .iter()
                    .map(|criterion: &QualitativeCriterion| {
                        json!({
                            "criterion_id": criterion.id(),
                            "applicability": "applicable",
                            "rating": 3,
                            "rating_scale": criterion.rating_scale(),
                            "confidence": 0.75,
                            "evidence_ids": evidence_ids,
                            "rationale": "The bounded project criterion is supported by the cited evidence."
                        })
                    })
                    .collect::<Vec<_>>();
                let response: Value = json!({
                    "schema_version": AI_JUDGMENT_SCHEMA_VERSION,
                    "evaluation_version": request.evaluation_version(),
                    "rubric_version": request.rubric_version(),
                    "status": "complete",
                    "evidence_bundle_hash": request.evidence_bundle_hash(),
                    "privacy": {
                        "evidence_scope": request.evidence_scope(),
                        "external_transmission": request.external_transmission()
                    },
                    "judgments": judgments
                });
                serde_json::to_vec(&response).map_err(|_| ProviderError)
            }
        }
    }
}
