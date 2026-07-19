//! Shared helpers for the OpenAI adapter tests.
#![allow(dead_code)]

use std::{
    cell::RefCell,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use assay_ai_evaluator::{
    EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope, ExternalTransmission,
    HttpTransport, OpenAiConfig, OpenAiEvaluator, OutboundRequest, ProviderSecret,
    QualitativeRubric, SamplingConfig, SecretError, SecretName, SecretStore, TransportError,
    TransportResponse,
};
use assay_domain::EvidenceId;
use serde_json::{Value, json};

pub(crate) const SECRET_VALUE: &str = "sk-test-rotating-key-000000000000";

pub(crate) fn evidence_id(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

pub(crate) fn bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        vec![
            EvidenceDescriptor::new(
                evidence_id("evidence:readme:claim-4"),
                EvidenceKind::DocumentationClaim,
                "The README describes a repository analysis workflow.",
            )
            .unwrap(),
            EvidenceDescriptor::new(
                evidence_id("evidence:test:integration-2"),
                EvidenceKind::Test,
                "A cited integration test exercises the documented workflow.",
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

pub(crate) fn config() -> OpenAiConfig {
    OpenAiConfig {
        endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
        model: "gpt-4o-mini".to_owned(),
        secret_name: SecretName::new("OPENAI_API_KEY").unwrap(),
        sampling: SamplingConfig {
            temperature: 0.0,
            top_p: 1.0,
            max_output_tokens: 1024,
            seed: Some(7),
        },
        timeout: Duration::from_secs(30),
    }
}

pub(crate) struct FakeStore {
    pub result: Result<&'static str, SecretError>,
}

impl FakeStore {
    pub fn with_key(value: &'static str) -> Self {
        Self { result: Ok(value) }
    }

    pub fn unavailable() -> Self {
        Self {
            result: Err(SecretError::Unavailable),
        }
    }
}

impl SecretStore for FakeStore {
    fn load(&self, _name: &SecretName) -> Result<ProviderSecret, SecretError> {
        self.result
            .map(|value| ProviderSecret::new(value.to_owned()))
    }
}

pub(crate) fn judgment_body() -> String {
    let bundle = bundle();
    serde_json::to_string(&json!({
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
            "rating": 3,
            "rating_scale": 4,
            "confidence": 0.82,
            "evidence_ids": ["evidence:readme:claim-4", "evidence:test:integration-2"],
            "rationale": "The documented workflow is supported by cited implementation and test evidence."
        }]
    }))
    .unwrap()
}

pub(crate) fn chat_envelope(content: &str, usage: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": "chatcmpl-test",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": content } }],
        "usage": usage
    }))
    .unwrap()
}

pub(crate) fn usage(prompt: u64, completion: u64) -> Value {
    json!({
        "prompt_tokens": prompt,
        "completion_tokens": completion,
        "total_tokens": prompt + completion
    })
}

pub(crate) struct ScriptedTransport {
    pub status: u16,
    pub body: Vec<u8>,
    pub latency: Duration,
    pub error: Option<TransportError>,
    pub seen_auth: RefCell<Option<String>>,
    pub seen_body: RefCell<Option<Vec<u8>>>,
    pub calls: AtomicUsize,
}

impl ScriptedTransport {
    pub fn ok(body: Vec<u8>, latency: Duration) -> Self {
        Self {
            status: 200,
            body,
            latency,
            error: None,
            seen_auth: RefCell::new(None),
            seen_body: RefCell::new(None),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
            latency: Duration::from_millis(5),
            error: None,
            seen_auth: RefCell::new(None),
            seen_body: RefCell::new(None),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn failing(error: TransportError) -> Self {
        Self {
            status: 0,
            body: Vec::new(),
            latency: Duration::ZERO,
            error: Some(error),
            seen_auth: RefCell::new(None),
            seen_body: RefCell::new(None),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub fn seen_body(&self) -> Vec<u8> {
        self.seen_body.borrow().clone().unwrap_or_default()
    }

    pub fn seen_auth(&self) -> String {
        self.seen_auth.borrow().clone().unwrap_or_default()
    }
}

impl HttpTransport for ScriptedTransport {
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.seen_auth.borrow_mut() = request.authorization();
        *self.seen_body.borrow_mut() = Some(request.body().to_vec());
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(TransportResponse::new(
            self.status,
            self.body.clone(),
            self.latency,
        ))
    }
}

pub(crate) fn evaluator<S: SecretStore, T: HttpTransport>(
    store: S,
    transport: T,
) -> OpenAiEvaluator<S, T> {
    OpenAiEvaluator::new(QualitativeRubric::project_v1(), config(), store, transport)
}
