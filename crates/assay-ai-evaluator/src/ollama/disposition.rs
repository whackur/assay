use crate::EvaluationErrorKind;

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

const fn disposition(code: &'static str, retryable: bool) -> OllamaFailureDisposition {
    OllamaFailureDisposition { code, retryable }
}
