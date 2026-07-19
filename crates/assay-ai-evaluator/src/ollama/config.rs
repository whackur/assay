use std::time::Duration;

use reqwest::Url;

use crate::{
    EvaluationErrorKind, ProviderRequest, SecretName,
    api::{ApiProviderProfile, AuthorizationScheme, ProviderReply, SamplingConfig},
};

pub const OLLAMA_COMPATIBLE_PROVIDER_ID: &str = "ollama-openai-compatible-api-1";
pub const OLLAMA_COMPATIBLE_PROFILE: &str = "ollama-openai-compatible-project-rubric-1";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaConfigError;

impl std::fmt::Display for OllamaConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "Ollama base URL must be an HTTPS /v1 base (or unauthenticated local HTTP) without URL credentials, query, or fragment, and model must be non-empty",
        )
    }
}

impl std::error::Error for OllamaConfigError {}

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
        if !url_is_valid(&base, secret_name.is_some()) || !model_is_valid(model) {
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

// Local HTTP hosts permitted for unauthenticated Ollama endpoints.
fn is_local_http_host(host: Option<&str>) -> bool {
    matches!(
        host,
        Some("localhost" | "127.0.0.1" | "::1" | "host.docker.internal")
    )
}

fn url_is_valid(base: &Url, has_secret: bool) -> bool {
    let scheme = base.scheme();
    let local_http = scheme == "http" && is_local_http_host(base.host_str());
    matches!(scheme, "http" | "https")
        && !(has_secret && scheme != "https")
        && !(scheme == "http" && !local_http)
        && !base.cannot_be_a_base()
        && base.username().is_empty()
        && base.password().is_none()
        && base.query().is_none()
        && base.fragment().is_none()
        && base.path().trim_end_matches('/') == "/v1"
}

fn model_is_valid(model: &str) -> bool {
    !model.trim().is_empty() && model.len() <= 255 && !model.chars().any(char::is_control)
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
        serde_json::to_vec(&serde_json::json!({
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
        let envelope: super::envelope::ChatEnvelope =
            serde_json::from_slice(body).map_err(|_| EvaluationErrorKind::MalformedOutput)?;
        let judgment = envelope
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or(EvaluationErrorKind::MalformedOutput)?;
        Ok(ProviderReply {
            judgment,
            usage: envelope
                .usage
                .and_then(super::envelope::ChatUsage::validated),
        })
    }
}

pub(crate) const MAX_RESPONSE_BYTES_EXPOSED: usize = MAX_RESPONSE_BYTES;
