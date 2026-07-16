//! Server-managed OpenAI API adapter over narrow credential and transport ports.
//!
//! The adapter turns a [`ProviderRequest`] into an OpenAI chat request, sends it
//! through an injected [`HttpTransport`], and validates the untrusted response
//! with the existing [`Evaluator`]. The concrete HTTP client and the concrete
//! secret store live outside this crate; only their seams and a deterministic
//! test double exercise the logic here, so no network, process, or credential
//! I/O happens in this crate. Provider, model, prompt/rubric versions, sampling,
//! and validation status are deterministic provenance; token usage and latency
//! are isolated non-deterministic telemetry excluded from score calculation.

use std::{fmt, time::Duration};

use serde_json::{Value, json};

use crate::{
    EvaluationErrorKind, Evaluator, EvidenceBundle, PROMPT_VERSION, ProviderExecutionBoundary,
    ProviderRequest, QualitativeRubric, ValidatedJudgmentSet,
    evaluator::enforce_transmission_boundary,
};

const PROVIDER_ID: &str = "openai-api-1";

/// Stable reference name of a secret, never the secret value itself.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecretName(String);

impl SecretName {
    /// Validates a secret reference name; rejects empty or unsafe names.
    pub fn new(name: &str) -> Result<Self, SecretError> {
        let valid = !name.is_empty()
            && name.len() <= 128
            && name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'/')
            });
        if valid {
            Ok(Self(name.to_owned()))
        } else {
            Err(SecretError::InvalidName)
        }
    }

    /// Returns the reference name used to look the secret up.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A loaded API credential that never appears in Debug, Display, or serialization.
#[derive(Clone)]
pub struct ProviderSecret(String);

impl ProviderSecret {
    /// Wraps raw key material read from a secret store.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Exposes the key only to code that builds the outbound request header.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderSecret(<redacted>)")
    }
}

/// Redacted failure category for secret resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretError {
    InvalidName,
    NotFound,
    Unavailable,
}

/// Name-addressed secret store; a rotated key is read by the same name.
pub trait SecretStore {
    /// Loads current key material for one reference name from secret storage.
    fn load(&self, name: &SecretName) -> Result<ProviderSecret, SecretError>;
}

/// One outbound HTTP request. Debug and Display never reveal the credential.
pub struct OutboundRequest {
    endpoint: String,
    body: Vec<u8>,
    timeout: Duration,
    secret: ProviderSecret,
}

impl OutboundRequest {
    /// Returns the fixed request endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Returns the request body bytes, which never contain the credential.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns the request timeout budget.
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns the `Authorization` header value; the only credential exposure.
    pub fn authorization(&self) -> String {
        format!("Bearer {}", self.secret.expose())
    }
}

impl fmt::Debug for OutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutboundRequest")
            .field("endpoint", &self.endpoint)
            .field("body_len", &self.body.len())
            .field("timeout", &self.timeout)
            .field("authorization", &"<redacted>")
            .finish()
    }
}

/// A completed transport response. The status and body are untrusted.
pub struct TransportResponse {
    status: u16,
    body: Vec<u8>,
    latency: Duration,
}

impl TransportResponse {
    /// Builds a response from an observed status, body, and measured latency.
    pub fn new(status: u16, body: Vec<u8>, latency: Duration) -> Self {
        Self {
            status,
            body,
            latency,
        }
    }
}

impl fmt::Debug for TransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .field("latency", &self.latency)
            .finish()
    }
}

/// Redacted transport failure with no request or response text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    Timeout,
    Network,
}

/// HTTP transport seam. The concrete client lives outside this crate.
pub trait HttpTransport {
    /// Sends one outbound request and returns an untrusted response.
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError>;
}

/// Deterministic sampling settings recorded as scoring-independent provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_p: f32,
    pub max_output_tokens: u32,
    pub seed: Option<u64>,
}

/// Server-side configuration for the OpenAI adapter.
#[derive(Clone, Debug)]
pub struct OpenAiConfig {
    pub endpoint: String,
    pub model: String,
    pub secret_name: SecretName,
    pub sampling: SamplingConfig,
    pub timeout: Duration,
}

/// Token usage reported by the provider. Non-deterministic, excluded from scores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Non-deterministic call telemetry isolated from deterministic judgment inputs.
#[derive(Clone, Copy, Debug)]
pub struct ProviderTelemetry {
    http_status: u16,
    latency: Duration,
    usage: Option<Usage>,
}

impl ProviderTelemetry {
    /// Returns the observed HTTP status.
    pub const fn http_status(&self) -> u16 {
        self.http_status
    }

    /// Returns the measured request latency.
    pub const fn latency(&self) -> Duration {
        self.latency
    }

    /// Returns provider-reported token usage when present.
    pub const fn usage(&self) -> Option<Usage> {
        self.usage
    }
}

/// Deterministic provenance recorded for every evaluation snapshot.
#[derive(Clone, Debug)]
pub struct SnapshotProvenance {
    provider_id: &'static str,
    model: String,
    prompt_version: &'static str,
    rubric_version: &'static str,
    evaluation_version: &'static str,
    sampling: SamplingConfig,
    evidence_bundle_hash: String,
}

impl SnapshotProvenance {
    /// Returns the stable provider adapter identifier.
    pub const fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    /// Returns the recorded provider model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the shared prompt-envelope version.
    pub const fn prompt_version(&self) -> &'static str {
        self.prompt_version
    }

    /// Returns the rubric version bound to the request.
    pub const fn rubric_version(&self) -> &'static str {
        self.rubric_version
    }

    /// Returns the evaluation version bound to the request.
    pub const fn evaluation_version(&self) -> &'static str {
        self.evaluation_version
    }

    /// Returns the recorded sampling configuration.
    pub const fn sampling(&self) -> SamplingConfig {
        self.sampling
    }

    /// Returns the exact evidence-bundle hash presented to the provider.
    pub fn evidence_bundle_hash(&self) -> &str {
        &self.evidence_bundle_hash
    }
}

/// Explicit validation status: a validated judgment set or a named failure.
#[derive(Clone, Debug)]
pub enum SnapshotOutcome {
    Validated(ValidatedJudgmentSet),
    Failed(EvaluationErrorKind),
}

impl SnapshotOutcome {
    /// Returns a stable status code that never disguises a failure as success.
    pub const fn status_code(&self) -> &'static str {
        match self {
            Self::Validated(_) => "validated",
            Self::Failed(kind) => kind.code(),
        }
    }
}

/// An honest, self-describing record of one provider evaluation attempt.
#[derive(Clone, Debug)]
pub struct EvaluationSnapshot {
    provenance: SnapshotProvenance,
    outcome: SnapshotOutcome,
    telemetry: Option<ProviderTelemetry>,
}

impl EvaluationSnapshot {
    /// Returns deterministic provenance recorded regardless of outcome.
    pub const fn provenance(&self) -> &SnapshotProvenance {
        &self.provenance
    }

    /// Returns the explicit validation outcome.
    pub const fn outcome(&self) -> &SnapshotOutcome {
        &self.outcome
    }

    /// Returns isolated non-deterministic telemetry, absent when no call completed.
    pub const fn telemetry(&self) -> Option<&ProviderTelemetry> {
        self.telemetry.as_ref()
    }

    /// Returns the validated judgment set only on the success path.
    pub const fn validated(&self) -> Option<&ValidatedJudgmentSet> {
        match &self.outcome {
            SnapshotOutcome::Validated(set) => Some(set),
            SnapshotOutcome::Failed(_) => None,
        }
    }
}

/// Server-side OpenAI adapter binding a rubric, config, secret store, and transport.
pub struct OpenAiEvaluator<S, T> {
    evaluator: Evaluator,
    config: OpenAiConfig,
    secret_store: S,
    transport: T,
}

impl<S: SecretStore, T: HttpTransport> OpenAiEvaluator<S, T> {
    /// Builds an adapter for one immutable rubric and provider configuration.
    pub const fn new(
        rubric: QualitativeRubric,
        config: OpenAiConfig,
        secret_store: S,
        transport: T,
    ) -> Self {
        Self {
            evaluator: Evaluator::new(rubric),
            config,
            secret_store,
            transport,
        }
    }

    /// Returns the injected transport, primarily for deployment introspection.
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Evaluates a bundle and always returns an explicit, recorded snapshot.
    pub fn evaluate(&self, bundle: &EvidenceBundle) -> EvaluationSnapshot {
        let provenance = self.provenance(bundle);
        if let Err(error) =
            enforce_transmission_boundary(ProviderExecutionBoundary::External, bundle)
        {
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
        };
        self.interpret(provenance, response, bundle)
    }

    fn provenance(&self, bundle: &EvidenceBundle) -> SnapshotProvenance {
        SnapshotProvenance {
            provider_id: PROVIDER_ID,
            model: self.config.model.clone(),
            prompt_version: PROMPT_VERSION,
            rubric_version: self.evaluator.rubric().version(),
            evaluation_version: self.evaluator.rubric().evaluation_version(),
            sampling: self.config.sampling,
            evidence_bundle_hash: bundle.content_hash().to_owned(),
        }
    }

    fn build_request(
        &self,
        request: &ProviderRequest<'_>,
    ) -> Result<OutboundRequest, EvaluationErrorKind> {
        let sampling = &self.config.sampling;
        let payload: Value = serde_json::from_str(request.canonical_payload())
            .map_err(|_| EvaluationErrorKind::SchemaInvalid)?;
        let mut body = json!({
            "model": self.config.model,
            "temperature": sampling.temperature,
            "top_p": sampling.top_p,
            "max_tokens": sampling.max_output_tokens,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": request.system_instructions() },
                { "role": "user", "content": payload }
            ]
        });
        if let Some(seed) = sampling.seed {
            body["seed"] = json!(seed);
        }
        let body = serde_json::to_vec(&body).map_err(|_| EvaluationErrorKind::SchemaInvalid)?;
        let secret = self
            .secret_store
            .load(&self.config.secret_name)
            .map_err(|_| EvaluationErrorKind::SecretUnavailable)?;
        Ok(OutboundRequest {
            endpoint: self.config.endpoint.clone(),
            body,
            timeout: self.config.timeout,
            secret,
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
        if let Some(kind) = http_status_failure(status) {
            let telemetry = ProviderTelemetry {
                http_status: status,
                latency,
                usage: parse_usage(&response.body),
            };
            return self.failed(provenance, kind, Some(telemetry));
        }
        let (content, usage) = match extract_content(&response.body) {
            Ok(parts) => parts,
            Err(kind) => {
                let telemetry = ProviderTelemetry {
                    http_status: status,
                    latency,
                    usage: None,
                };
                return self.failed(provenance, kind, Some(telemetry));
            }
        };
        let telemetry = ProviderTelemetry {
            http_status: status,
            latency,
            usage,
        };
        match self.evaluator.validate_bytes(content.as_bytes(), bundle) {
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

const fn http_status_failure(status: u16) -> Option<EvaluationErrorKind> {
    match status {
        200 => None,
        401 | 403 => Some(EvaluationErrorKind::ProviderUnauthorized),
        429 => Some(EvaluationErrorKind::ProviderRateLimited),
        _ => Some(EvaluationErrorKind::ProviderFailure),
    }
}

fn extract_content(body: &[u8]) -> Result<(String, Option<Usage>), EvaluationErrorKind> {
    let envelope: Value =
        serde_json::from_slice(body).map_err(|_| EvaluationErrorKind::MalformedOutput)?;
    let content = envelope
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or(EvaluationErrorKind::MalformedOutput)?;
    Ok((content.to_owned(), parse_usage(body)))
}

fn parse_usage(body: &[u8]) -> Option<Usage> {
    let envelope: Value = serde_json::from_slice(body).ok()?;
    let usage = envelope.get("usage")?;
    let field = |name: &str| -> Option<u32> {
        usage
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    Some(Usage {
        prompt_tokens: field("prompt_tokens")?,
        completion_tokens: field("completion_tokens")?,
        total_tokens: field("total_tokens")?,
    })
}
