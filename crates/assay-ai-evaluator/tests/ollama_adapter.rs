use std::{cell::RefCell, time::Duration};

use assay_ai_evaluator::{
    HttpTransport, OllamaCompatibleConfig, OllamaCompatibleEvaluator, OutboundRequest,
    ProviderSecret, SecretError, SecretName, SecretStore, SnapshotOutcome, TransportError,
    TransportResponse, build_hosted_metadata_bundle, classify_ollama_failure,
};
use serde_json::{Value, json};

struct NoSecretStore;

impl SecretStore for NoSecretStore {
    fn load(&self, _name: &SecretName) -> Result<ProviderSecret, SecretError> {
        Err(SecretError::Unavailable)
    }
}

struct ScriptedTransport {
    status: u16,
    response: Vec<u8>,
    retry_after: Option<Duration>,
    seen_body: RefCell<Vec<u8>>,
}

impl ScriptedTransport {
    fn new(response: Vec<u8>) -> Self {
        Self {
            status: 200,
            response,
            retry_after: None,
            seen_body: RefCell::new(Vec::new()),
        }
    }

    fn rate_limited(seconds: u64) -> Self {
        Self {
            status: 429,
            response: Vec::new(),
            retry_after: Some(Duration::from_secs(seconds)),
            seen_body: RefCell::new(Vec::new()),
        }
    }
}

impl HttpTransport for ScriptedTransport {
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError> {
        *self.seen_body.borrow_mut() = request.body().to_vec();
        Ok(
            TransportResponse::new(self.status, self.response.clone(), Duration::from_millis(5))
                .with_retry_after(self.retry_after),
        )
    }
}

fn facts() -> Value {
    json!({
        "description": "not transmitted as evaluation evidence",
        "stargazers_count": 12,
        "forks_count": 3,
        "open_issues_count": 2,
        "archived": false,
        "fork": false,
        "license_spdx": "MIT",
        "default_branch": "main",
        "head_sha": "0123456789abcdef0123456789abcdef01234567",
        "full_name": "whackur/assay"
    })
}

fn config() -> OllamaCompatibleConfig {
    OllamaCompatibleConfig::from_base_url("http://localhost:11434/v1", "qwen3", None).unwrap()
}

fn judgment(extra: Option<(&str, Value)>, cited: bool) -> String {
    let bundle = build_hosted_metadata_bundle(&facts()).unwrap();
    let mut value = json!({
        "schema_version": "1.0.0",
        "evaluation_version": "project-intelligence-1",
        "rubric_version": "project-rubric-1",
        "status": "partial",
        "evidence_bundle_hash": bundle.content_hash(),
        "privacy": {
            "evidence_scope": "public_only",
            "external_transmission": "public_only"
        },
        "judgments": [{
            "criterion_id": "substance.claim_implementation_fit",
            "applicability": "applicable",
            "rating": 2,
            "rating_scale": 4,
            "confidence": 0.7,
            "evidence_ids": if cited { json!([bundle.items()[0].id().as_str()]) } else { json!([]) },
            "rationale": "The bounded GitHub metadata supports only a limited qualitative observation."
        }]
    });
    if let Some((key, item)) = extra {
        value[key] = item;
    }
    serde_json::to_string(&value).unwrap()
}

fn envelope(content: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": "chatcmpl-test",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30}
    }))
    .unwrap()
}

#[test]
fn config_pins_exact_openai_compatible_endpoint() {
    assert_eq!(
        config().endpoint(),
        "http://localhost:11434/v1/chat/completions"
    );
    assert!(
        OllamaCompatibleConfig::from_base_url("https://ollama.invalid/api", "qwen3", None).is_err()
    );
    assert!(
        OllamaCompatibleConfig::from_base_url(
            "https://user:secret@ollama.invalid/v1",
            "qwen3",
            None
        )
        .is_err()
    );
    assert!(
        OllamaCompatibleConfig::from_base_url(
            "http://localhost:11434/v1",
            "qwen3",
            Some(SecretName::new("ASSAY_OLLAMA_API_KEY").unwrap())
        )
        .is_err()
    );
    assert!(
        OllamaCompatibleConfig::from_base_url("http://ollama.invalid/v1", "qwen3", None).is_err()
    );
    assert!(
        OllamaCompatibleConfig::from_base_url(
            "https://ollama.invalid/v1",
            "qwen3",
            Some(SecretName::new("ASSAY_OLLAMA_API_KEY").unwrap())
        )
        .is_ok()
    );
    assert!(
        OllamaCompatibleConfig::from_base_url(
            "http://host.docker.internal:11434/v1",
            "qwen3",
            None
        )
        .is_ok()
    );
}

#[test]
fn request_is_non_streaming_and_carries_canonical_payload_as_text() {
    let transport = ScriptedTransport::new(envelope(&judgment(None, true)));
    let evaluator = OllamaCompatibleEvaluator::new(config(), NoSecretStore, transport);
    let snapshot = evaluator.evaluate_hosted_metadata(&facts()).unwrap();
    assert!(matches!(snapshot.outcome(), SnapshotOutcome::Validated(_)));
    assert_eq!(
        snapshot.provenance().provider_id(),
        "ollama-openai-compatible-api-1"
    );

    let request: Value = serde_json::from_slice(&evaluator.transport().seen_body.borrow()).unwrap();
    assert_eq!(request["stream"], false);
    assert_eq!(request["response_format"]["type"], "json_object");
    assert!(request["messages"][1]["content"].is_string());
    let payload: Value =
        serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert!(payload["begin_evidence"].is_array());
    assert!(
        !request["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("not transmitted")
    );
}

#[test]
fn unknown_fields_and_missing_citations_fail_closed() {
    let unknown = ScriptedTransport::new(envelope(&judgment(Some(("score", json!(99))), true)));
    let evaluator = OllamaCompatibleEvaluator::new(config(), NoSecretStore, unknown);
    let snapshot = evaluator.evaluate_hosted_metadata(&facts()).unwrap();
    assert!(
        matches!(snapshot.outcome(), SnapshotOutcome::Failed(kind) if kind.code() == "schema_invalid")
    );

    let uncited = ScriptedTransport::new(envelope(&judgment(None, false)));
    let evaluator = OllamaCompatibleEvaluator::new(config(), NoSecretStore, uncited);
    let snapshot = evaluator.evaluate_hosted_metadata(&facts()).unwrap();
    assert!(
        matches!(snapshot.outcome(), SnapshotOutcome::Failed(kind) if kind.code() == "missing_citation")
    );
}

#[test]
fn response_bounds_and_failure_policy_are_owned_by_adapter() {
    let oversized = ScriptedTransport::new(vec![b'x'; 256 * 1024 + 1]);
    let evaluator = OllamaCompatibleEvaluator::new(config(), NoSecretStore, oversized);
    let snapshot = evaluator.evaluate_hosted_metadata(&facts()).unwrap();
    let kind = match snapshot.outcome() {
        SnapshotOutcome::Failed(kind) => *kind,
        other => panic!("expected failure, got {other:?}"),
    };
    let disposition = classify_ollama_failure(kind);
    assert_eq!(disposition.code(), "ollama_response_too_large");
    assert!(!disposition.retryable());
}

#[test]
fn provider_retry_after_is_preserved_in_attempt_telemetry() {
    let evaluator = OllamaCompatibleEvaluator::new(
        config(),
        NoSecretStore,
        ScriptedTransport::rate_limited(900),
    );
    let snapshot = evaluator.evaluate_hosted_metadata(&facts()).unwrap();
    assert!(matches!(snapshot.outcome(), SnapshotOutcome::Failed(_)));
    assert_eq!(
        snapshot.telemetry().and_then(|value| value.retry_after()),
        Some(Duration::from_secs(900))
    );
}
