//! Ollama/OpenAI-compatible hosted evaluator over the shared API-key family.

use std::{
    error::Error,
    fmt,
    io::Read,
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use assay_domain::EvidenceId;
use assay_project_intelligence::{
    HostedEvaluationAttempt, HostedEvaluationInput, HostedEvaluationPort, HostedFailure,
};
use reqwest::{Url, blocking::Client, header};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    EvaluationError, EvaluationErrorKind, EvaluationSnapshot, EvidenceBundle, EvidenceDescriptor,
    EvidenceKind, EvidenceScope, ExternalTransmission, HttpTransport, OutboundRequest,
    ProviderRequest, QualitativeRubric, SecretName, SecretStore, TransportError, TransportResponse,
    Usage,
    api::{
        ApiKeyEvaluator, ApiProviderProfile, AuthorizationScheme, ProviderReply, SamplingConfig,
        SnapshotOutcome,
    },
};

pub const OLLAMA_COMPATIBLE_PROVIDER_ID: &str = "ollama-openai-compatible-api-1";
pub const OLLAMA_COMPATIBLE_PROFILE: &str = "ollama-openai-compatible-project-rubric-1";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaConfigError;

impl fmt::Display for OllamaConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Ollama base URL must be an HTTPS /v1 base (or unauthenticated local HTTP) without URL credentials, query, or fragment, and model must be non-empty",
        )
    }
}

impl Error for OllamaConfigError {}

/// Validated immutable Ollama-compatible provider configuration.
#[derive(Clone, Debug)]
pub struct OllamaCompatibleConfig {
    endpoint: String,
    model: String,
    secret_name: Option<SecretName>,
    sampling: SamplingConfig,
    timeout: Duration,
}

impl OllamaCompatibleConfig {
    pub fn from_base_url(
        base_url: &str,
        model: &str,
        secret_name: Option<SecretName>,
    ) -> Result<Self, OllamaConfigError> {
        let base = Url::parse(base_url).map_err(|_| OllamaConfigError)?;
        let local_http = base.scheme() == "http"
            && matches!(
                base.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "host.docker.internal")
            );
        if !matches!(base.scheme(), "http" | "https")
            || (secret_name.is_some() && base.scheme() != "https")
            || (base.scheme() == "http" && !local_http)
            || base.cannot_be_a_base()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || base.path().trim_end_matches('/') != "/v1"
            || model.trim().is_empty()
            || model.len() > 255
            || model.chars().any(char::is_control)
        {
            return Err(OllamaConfigError);
        }
        Ok(Self {
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
            model: model.to_owned(),
            secret_name,
            sampling: SamplingConfig {
                temperature: 0.0,
                top_p: 1.0,
                max_output_tokens: 2_048,
                seed: None,
            },
            timeout: Duration::from_secs(45),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Clone, Debug)]
pub struct OllamaProfile {
    config: OllamaCompatibleConfig,
}

impl OllamaProfile {
    pub const fn new(config: OllamaCompatibleConfig) -> Self {
        Self { config }
    }
}

impl ApiProviderProfile for OllamaProfile {
    fn provider_id(&self) -> &'static str {
        OLLAMA_COMPATIBLE_PROVIDER_ID
    }

    fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn secret_name(&self) -> Option<&SecretName> {
        self.config.secret_name.as_ref()
    }

    fn sampling(&self) -> SamplingConfig {
        self.config.sampling
    }

    fn timeout(&self) -> Duration {
        self.config.timeout
    }

    fn authorization(&self) -> Option<AuthorizationScheme> {
        self.config
            .secret_name
            .as_ref()
            .map(|_| AuthorizationScheme {
                header_name: "Authorization",
                value_prefix: "Bearer ",
            })
    }

    fn request_body(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, EvaluationErrorKind> {
        let sampling = self.config.sampling;
        serde_json::to_vec(&json!({
            "model": self.config.model,
            "stream": false,
            "temperature": sampling.temperature,
            "top_p": sampling.top_p,
            "max_tokens": sampling.max_output_tokens,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role": "system", "content": request.system_instructions()},
                {"role": "user", "content": request.canonical_payload()}
            ]
        }))
        .map_err(|_| EvaluationErrorKind::SchemaInvalid)
    }

    fn classify_http_status(&self, status: u16) -> Option<EvaluationErrorKind> {
        match status {
            200 => None,
            401 | 403 => Some(EvaluationErrorKind::ProviderUnauthorized),
            429 => Some(EvaluationErrorKind::ProviderRateLimited),
            404 | 405 | 422 => Some(EvaluationErrorKind::SchemaInvalid),
            _ => Some(EvaluationErrorKind::ProviderFailure),
        }
    }

    fn extract_reply(&self, body: &[u8]) -> Result<ProviderReply, EvaluationErrorKind> {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(EvaluationErrorKind::OutputTooLarge);
        }
        let envelope: ChatEnvelope =
            serde_json::from_slice(body).map_err(|_| EvaluationErrorKind::MalformedOutput)?;
        let judgment = envelope
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or(EvaluationErrorKind::MalformedOutput)?;
        Ok(ProviderReply {
            judgment,
            usage: envelope.usage.and_then(ChatUsage::validated),
        })
    }
}

#[derive(Deserialize)]
struct ChatEnvelope {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatChoice {
    #[serde(default)]
    #[serde(rename = "index")]
    _index: Option<u32>,
    message: ChatMessage,
    #[serde(default)]
    #[serde(rename = "finish_reason")]
    _finish_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatMessage {
    #[serde(default)]
    #[serde(rename = "role")]
    _role: Option<String>,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl ChatUsage {
    fn validated(self) -> Option<Usage> {
        (self.prompt_tokens.checked_add(self.completion_tokens)? == self.total_tokens).then_some(
            Usage {
                prompt_tokens: self.prompt_tokens,
                completion_tokens: self.completion_tokens,
                total_tokens: self.total_tokens,
            },
        )
    }
}

/// Shared evaluator plus Ollama-specific immutable profile.
pub struct OllamaCompatibleEvaluator<S, T> {
    inner: ApiKeyEvaluator<OllamaProfile, S, T>,
}

impl<S: SecretStore, T: HttpTransport> OllamaCompatibleEvaluator<S, T> {
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

/// Builds only bounded deterministic facts; repository descriptions and raw provider text are excluded.
pub fn build_hosted_metadata_bundle(facts: &Value) -> Result<EvidenceBundle, EvaluationError> {
    let mut items = Vec::new();
    push_u64(&mut items, facts, "stargazers_count", "stargazers");
    push_u64(&mut items, facts, "forks_count", "forks");
    push_u64(&mut items, facts, "open_issues_count", "open-issues");
    push_bool(&mut items, facts, "archived", "archived");
    push_bool(&mut items, facts, "fork", "fork");
    if let Some(value) = facts.get("head_sha").and_then(Value::as_str)
        && value.len() == 40
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        push_descriptor(
            &mut items,
            "evidence:github:head-sha",
            &format!(
                "GitHub resolved the default branch to commit {}.",
                value.to_ascii_lowercase()
            ),
        );
    }
    if let Some(value) = facts.get("license_spdx").and_then(Value::as_str)
        && !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'+'))
    {
        push_descriptor(
            &mut items,
            "evidence:github:license-spdx",
            &format!("GitHub reports the SPDX license identifier {value}."),
        );
    }
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items,
    )
}

fn push_u64(items: &mut Vec<EvidenceDescriptor>, facts: &Value, key: &str, id: &str) {
    if let Some(value) = facts.get(key).and_then(Value::as_u64) {
        push_descriptor(
            items,
            &format!("evidence:github:{id}"),
            &format!("GitHub reports {value} for {key}."),
        );
    }
}

fn push_bool(items: &mut Vec<EvidenceDescriptor>, facts: &Value, key: &str, id: &str) {
    if let Some(value) = facts.get(key).and_then(Value::as_bool) {
        push_descriptor(
            items,
            &format!("evidence:github:{id}"),
            &format!("GitHub reports {key} as {value}."),
        );
    }
}

fn push_descriptor(items: &mut Vec<EvidenceDescriptor>, id: &str, statement: &str) {
    let id = EvidenceId::from_str(id).expect("hard-coded evidence identifier is valid");
    if let Ok(descriptor) = EvidenceDescriptor::new(id, EvidenceKind::RepositoryFact, statement) {
        items.push(descriptor);
    }
}

/// Concrete bounded HTTP transport owned by the provider adapter crate.
pub struct OllamaCompatibleHttpTransport {
    client: Client,
}

impl OllamaCompatibleHttpTransport {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
}

impl HttpTransport for OllamaCompatibleHttpTransport {
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError> {
        let started = Instant::now();
        let mut outbound = self
            .client
            .post(request.endpoint())
            .timeout(request.timeout())
            .header(header::CONTENT_TYPE, "application/json")
            .body(request.body().to_vec());
        if let (Some(name), Some(value)) =
            (request.authorization_header_name(), request.authorization())
        {
            outbound = outbound.header(name, value);
        }
        let mut response = outbound.send().map_err(|error| {
            if error.is_timeout() {
                TransportError::Timeout
            } else {
                TransportError::Network
            }
        })?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(TransportError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let retry_after = provider_retry_delay(response.headers());
        let mut body = Vec::new();
        response
            .by_ref()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| TransportError::Network)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(TransportError::ResponseTooLarge);
        }
        Ok(TransportResponse::new(status, body, started.elapsed()).with_retry_after(retry_after))
    }
}

fn provider_retry_delay(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let retry_after = headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let reset_delay = headers
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|reset| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
            Some(reset.saturating_sub(now))
        });
    retry_after.max(reset_delay).map(Duration::from_secs)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaFailureDisposition {
    code: &'static str,
    retryable: bool,
}

impl OllamaFailureDisposition {
    pub const fn code(self) -> &'static str {
        self.code
    }

    pub const fn retryable(self) -> bool {
        self.retryable
    }
}

pub const fn classify_ollama_failure(kind: EvaluationErrorKind) -> OllamaFailureDisposition {
    match kind {
        EvaluationErrorKind::ProviderTimeout => disposition("ollama_timeout", true),
        EvaluationErrorKind::ProviderRateLimited => disposition("ollama_rate_limited", true),
        EvaluationErrorKind::ProviderFailure => disposition("ollama_provider_failure", true),
        EvaluationErrorKind::ProviderUnauthorized => disposition("ollama_unauthorized", false),
        EvaluationErrorKind::SecretUnavailable => disposition("ollama_secret_unavailable", false),
        EvaluationErrorKind::OutputTooLarge => disposition("ollama_response_too_large", false),
        EvaluationErrorKind::MalformedOutput => disposition("ollama_malformed_response", false),
        EvaluationErrorKind::SchemaInvalid => disposition("ollama_openai_contract_invalid", false),
        _ => disposition("ollama_judgment_invalid", false),
    }
}

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
                SnapshotOutcome::Validated(_) => {
                    Ok(workflow_attempt(&snapshot, "validated_unpublished", None))
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
                        )));
                        Err(failure)
                    } else {
                        Ok(workflow_attempt(
                            &snapshot,
                            "unavailable",
                            Some(disposition.code()),
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
    }
}

fn workflow_attempt(
    snapshot: &EvaluationSnapshot,
    status: &str,
    error_code: Option<&str>,
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
    }
}

const fn disposition(code: &'static str, retryable: bool) -> OllamaFailureDisposition {
    OllamaFailureDisposition { code, retryable }
}
