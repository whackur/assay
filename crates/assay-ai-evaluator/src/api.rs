//! Shared machinery for the API-key HTTP provider family (ADR 0012).
//!
//! Every API-key provider reuses the injected [`SecretStore`] and
//! [`HttpTransport`] ports, the canonical [`ProviderRequest`] payload, the
//! [`EvaluationSnapshot`] record, and the shared failure taxonomy. The parts
//! that differ per provider — endpoint, model, request envelope, authorization
//! header form, HTTP status classification, and response extraction — are
//! isolated behind [`ApiProviderProfile`], so a new API provider is a new
//! envelope builder and extractor over the same two ports, never new
//! validation logic. Untrusted provider bytes always pass through the one
//! [`Evaluator`] validator.

use std::{fmt, time::Duration};

use crate::{
    EvaluationErrorKind, Evaluator, EvidenceBundle, PROMPT_VERSION, ProviderExecutionBoundary,
    ProviderRequest, QualitativeRubric, TransmissionSurface, ValidatedJudgmentSet,
    evaluator::enforce_transmission_boundary,
};

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

/// The provider-specific authorization header form, without the credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorizationScheme {
    /// The HTTP header carrying the credential, for example `Authorization`.
    pub header_name: &'static str,
    /// The fixed value prefix before the key material, for example `Bearer `.
    pub value_prefix: &'static str,
}

/// One outbound HTTP request. Debug and Display never reveal the credential.
pub struct OutboundRequest {
    endpoint: String,
    body: Vec<u8>,
    timeout: Duration,
    header_name: &'static str,
    authorization: ProviderSecret,
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

    /// Returns the header name carrying the credential.
    pub const fn authorization_header_name(&self) -> &'static str {
        self.header_name
    }

    /// Returns the authorization header value; the only credential exposure.
    pub fn authorization(&self) -> String {
        self.authorization.expose().to_owned()
    }
}

impl fmt::Debug for OutboundRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutboundRequest")
            .field("endpoint", &self.endpoint)
            .field("body_len", &self.body.len())
            .field("timeout", &self.timeout)
            .field("authorization_header_name", &self.header_name)
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

/// Token usage reported by the provider. Non-deterministic, excluded from scores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// The judgment text and telemetry a profile extracted from a response body.
#[derive(Debug)]
pub struct ProviderReply {
    /// The untrusted judgment document text for the one shared validator.
    pub judgment: String,
    /// Provider-reported token usage, when present.
    pub usage: Option<Usage>,
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

/// The per-provider parts of an API-key adapter: identity, envelope building,
/// authorization form, status classification, and response extraction.
///
/// Implementations hold their own `Config` and never perform I/O; the shared
/// [`ApiKeyEvaluator`] drives the two injected ports and the one validator.
pub trait ApiProviderProfile {
    /// Returns the stable adapter identifier recorded as provenance.
    fn provider_id(&self) -> &'static str;

    /// Returns the fixed request endpoint.
    fn endpoint(&self) -> &str;

    /// Returns the provider model identifier recorded as provenance.
    fn model(&self) -> &str;

    /// Returns the reference name of the provider credential.
    fn secret_name(&self) -> &SecretName;

    /// Returns the deterministic sampling settings recorded as provenance.
    fn sampling(&self) -> SamplingConfig;

    /// Returns the transport timeout budget.
    fn timeout(&self) -> Duration;

    /// Returns the authorization header form; the key material never appears.
    fn authorization(&self) -> AuthorizationScheme;

    /// Builds the provider request body from the canonical payload. The body
    /// never contains the credential.
    fn request_body(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, EvaluationErrorKind>;

    /// Classifies a non-success HTTP status into the shared failure taxonomy.
    fn classify_http_status(&self, status: u16) -> Option<EvaluationErrorKind>;

    /// Extracts the untrusted judgment text and telemetry from a response body.
    fn extract_reply(&self, body: &[u8]) -> Result<ProviderReply, EvaluationErrorKind>;
}

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
        let secret = self
            .secret_store
            .load(self.profile.secret_name())
            .map_err(|_| EvaluationErrorKind::SecretUnavailable)?;
        let scheme = self.profile.authorization();
        let authorization =
            ProviderSecret::new(format!("{}{}", scheme.value_prefix, secret.expose()));
        Ok(OutboundRequest {
            endpoint: self.profile.endpoint().to_owned(),
            body,
            timeout: self.profile.timeout(),
            header_name: scheme.header_name,
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
        if let Some(kind) = self.profile.classify_http_status(status) {
            let usage = self
                .profile
                .extract_reply(&response.body)
                .ok()
                .and_then(|reply| reply.usage);
            let telemetry = ProviderTelemetry {
                http_status: status,
                latency,
                usage,
            };
            return self.failed(provenance, kind, Some(telemetry));
        }
        let reply = match self.profile.extract_reply(&response.body) {
            Ok(reply) => reply,
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
            usage: reply.usage,
        };
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
