use crate::{
    EvaluationErrorKind, Evaluator, EvidenceBundle, PROMPT_VERSION, ProviderExecutionBoundary,
    ProviderRequest, QualitativeRubric, TransmissionSurface,
    evaluator::enforce_transmission_boundary,
};

use super::profile::ApiProviderProfile;
use super::provenance::{
    EvaluationSnapshot, ProviderTelemetry, SnapshotOutcome, SnapshotProvenance,
};
use super::secret::{ProviderSecret, SecretStore};
use super::transport::{HttpTransport, OutboundRequest, TransportError, TransportResponse};

/// Shared API-key family adapter binding a rubric, one provider profile, and
/// the injected credential and transport ports.
pub struct ApiKeyEvaluator<P, S, T> {
    evaluator: Evaluator,
    profile: P,
    secret_store: S,
    transport: T,
}

impl<P: ApiProviderProfile, S: SecretStore, T: HttpTransport> ApiKeyEvaluator<P, S, T> {
    /// Builds an adapter for one immutable rubric and provider profile.
    pub const fn new(rubric: QualitativeRubric, profile: P, secret_store: S, transport: T) -> Self {
        Self {
            evaluator: Evaluator::new(rubric),
            profile,
            secret_store,
            transport,
        }
    }

    /// Returns the injected transport, primarily for deployment introspection.
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Returns the provider profile bound to this adapter.
    pub const fn profile(&self) -> &P {
        &self.profile
    }

    /// Evaluates a bundle and always returns an explicit, recorded snapshot.
    pub fn evaluate(&self, bundle: &EvidenceBundle) -> EvaluationSnapshot {
        let provenance = self.provenance(bundle);
        if let Err(error) = enforce_transmission_boundary(
            ProviderExecutionBoundary::External,
            TransmissionSurface::BundleOnly,
            bundle,
        ) {
            return self.failed(provenance, error.kind(), None);
        }
        let request = match ProviderRequest::new(self.evaluator.rubric(), bundle) {
            Ok(request) => request,
            Err(error) => return self.failed(provenance, error.kind(), None),
        };
        let outbound = match self.build_request(&request) {
            Ok(outbound) => outbound,
            Err(kind) => return self.failed(provenance, kind, None),
        };
        let response = match self.transport.send(&outbound) {
            Ok(response) => response,
            Err(TransportError::Timeout) => {
                return self.failed(provenance, EvaluationErrorKind::ProviderTimeout, None);
            }
            Err(TransportError::Network) => {
                return self.failed(provenance, EvaluationErrorKind::ProviderFailure, None);
            }
            Err(TransportError::ResponseTooLarge) => {
                return self.failed(provenance, EvaluationErrorKind::OutputTooLarge, None);
            }
        };
        self.interpret(provenance, response, bundle)
    }

    fn provenance(&self, bundle: &EvidenceBundle) -> SnapshotProvenance {
        SnapshotProvenance {
            provider_id: self.profile.provider_id(),
            model: self.profile.model().to_owned(),
            prompt_version: PROMPT_VERSION,
            rubric_version: self.evaluator.rubric().version(),
            evaluation_version: self.evaluator.rubric().evaluation_version(),
            sampling: self.profile.sampling(),
            evidence_bundle_hash: bundle.content_hash().to_owned(),
        }
    }

    fn build_request(
        &self,
        request: &ProviderRequest<'_>,
    ) -> Result<OutboundRequest, EvaluationErrorKind> {
        let body = self.profile.request_body(request)?;
        let (header_name, authorization) =
            match (self.profile.secret_name(), self.profile.authorization()) {
                (Some(name), Some(scheme)) => {
                    let secret = self
                        .secret_store
                        .load(name)
                        .map_err(|_| EvaluationErrorKind::SecretUnavailable)?;
                    (
                        Some(scheme.header_name),
                        Some(ProviderSecret::new(format!(
                            "{}{}",
                            scheme.value_prefix,
                            secret.expose()
                        ))),
                    )
                }
                (None, None) => (None, None),
                _ => return Err(EvaluationErrorKind::SchemaInvalid),
            };
        Ok(OutboundRequest {
            endpoint: self.profile.endpoint().to_owned(),
            body,
            timeout: self.profile.timeout(),
            header_name,
            authorization,
        })
    }

    fn interpret(
        &self,
        provenance: SnapshotProvenance,
        response: TransportResponse,
        bundle: &EvidenceBundle,
    ) -> EvaluationSnapshot {
        let status = response.status;
        let latency = response.latency;
        let retry_after = response.retry_after;
        if let Some(kind) = self.profile.classify_http_status(status) {
            let usage = self
                .profile
                .extract_reply(&response.body)
                .ok()
                .and_then(|reply| reply.usage);
            let telemetry = ProviderTelemetry::from_response(status, latency, retry_after, usage);
            return self.failed(provenance, kind, Some(telemetry));
        }
        let reply = match self.profile.extract_reply(&response.body) {
            Ok(reply) => reply,
            Err(kind) => {
                let telemetry =
                    ProviderTelemetry::from_response(status, latency, retry_after, None);
                return self.failed(provenance, kind, Some(telemetry));
            }
        };
        let telemetry = ProviderTelemetry::from_response(status, latency, retry_after, reply.usage);
        match self
            .evaluator
            .validate_bytes(reply.judgment.as_bytes(), bundle)
        {
            Ok(set) => EvaluationSnapshot {
                provenance,
                outcome: SnapshotOutcome::Validated(set),
                telemetry: Some(telemetry),
            },
            Err(error) => self.failed(provenance, error.kind(), Some(telemetry)),
        }
    }

    fn failed(
        &self,
        provenance: SnapshotProvenance,
        kind: EvaluationErrorKind,
        telemetry: Option<ProviderTelemetry>,
    ) -> EvaluationSnapshot {
        EvaluationSnapshot {
            provenance,
            outcome: SnapshotOutcome::Failed(kind),
            telemetry,
        }
    }
}
