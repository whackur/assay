//! Server-managed OpenAI API adapter over the shared API-key family machinery.
//!
//! Only the OpenAI-specific parts live here: the chat request envelope, the
//! `Authorization: Bearer` header form, HTTP status classification, and the
//! `choices[0].message.content` response extraction, all expressed as an
//! [`ApiProviderProfile`]. Everything shared across API-key providers — the
//! injected [`SecretStore`](crate::SecretStore) and
//! [`HttpTransport`](crate::HttpTransport) ports, envelope assembly around the
//! canonical [`ProviderRequest`], the [`EvaluationSnapshot`] record, and the
//! failure taxonomy — comes from [`crate::api`], and the untrusted response is
//! validated by the one existing `Evaluator` path.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{
    EvaluationErrorKind, EvaluationSnapshot, EvidenceBundle, HttpTransport, ProviderRequest,
    QualitativeRubric, SecretName, SecretStore, Usage,
    api::{
        ApiKeyEvaluator, ApiProviderProfile, AuthorizationScheme, ProviderReply, SamplingConfig,
    },
};

const PROVIDER_ID: &str = "openai-api-1";

/// Server-side configuration for the OpenAI adapter.
#[derive(Clone, Debug)]
pub struct OpenAiConfig {
    pub endpoint: String,
    pub model: String,
    pub secret_name: SecretName,
    pub sampling: SamplingConfig,
    pub timeout: Duration,
}

/// The OpenAI-specific request envelope, authentication form, and response
/// extraction over the shared API-key family machinery.
#[derive(Clone, Debug)]
pub struct OpenAiProfile {
    config: OpenAiConfig,
}

impl OpenAiProfile {
    /// Binds one immutable provider configuration to the OpenAI envelope.
    pub const fn new(config: OpenAiConfig) -> Self {
        Self { config }
    }
}

impl ApiProviderProfile for OpenAiProfile {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn secret_name(&self) -> Option<&SecretName> {
        Some(&self.config.secret_name)
    }

    fn sampling(&self) -> SamplingConfig {
        self.config.sampling
    }

    fn timeout(&self) -> Duration {
        self.config.timeout
    }

    fn authorization(&self) -> Option<AuthorizationScheme> {
        Some(AuthorizationScheme {
            header_name: "Authorization",
            value_prefix: "Bearer ",
        })
    }

    fn request_body(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, EvaluationErrorKind> {
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
        serde_json::to_vec(&body).map_err(|_| EvaluationErrorKind::SchemaInvalid)
    }

    fn classify_http_status(&self, status: u16) -> Option<EvaluationErrorKind> {
        match status {
            200 => None,
            401 | 403 => Some(EvaluationErrorKind::ProviderUnauthorized),
            429 => Some(EvaluationErrorKind::ProviderRateLimited),
            _ => Some(EvaluationErrorKind::ProviderFailure),
        }
    }

    fn extract_reply(&self, body: &[u8]) -> Result<ProviderReply, EvaluationErrorKind> {
        let envelope: Value =
            serde_json::from_slice(body).map_err(|_| EvaluationErrorKind::MalformedOutput)?;
        let judgment = envelope
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str)
            .ok_or(EvaluationErrorKind::MalformedOutput)?
            .to_owned();
        Ok(ProviderReply {
            judgment,
            usage: parse_usage(&envelope),
        })
    }
}

/// Server-side OpenAI adapter binding a rubric, config, secret store, and transport.
pub struct OpenAiEvaluator<S, T> {
    inner: ApiKeyEvaluator<OpenAiProfile, S, T>,
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
            inner: ApiKeyEvaluator::new(
                rubric,
                OpenAiProfile::new(config),
                secret_store,
                transport,
            ),
        }
    }

    /// Returns the injected transport, primarily for deployment introspection.
    pub const fn transport(&self) -> &T {
        self.inner.transport()
    }

    /// Evaluates a bundle and always returns an explicit, recorded snapshot.
    pub fn evaluate(&self, bundle: &EvidenceBundle) -> EvaluationSnapshot {
        self.inner.evaluate(bundle)
    }
}

fn parse_usage(envelope: &Value) -> Option<Usage> {
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
