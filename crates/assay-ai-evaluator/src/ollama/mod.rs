//! Ollama/OpenAI-compatible hosted evaluator over the shared API-key family.

mod config;
mod disposition;
mod envelope;
mod evaluator;
mod metadata;
mod transport;
mod workflow;

pub use config::{
    OLLAMA_COMPATIBLE_PROFILE, OLLAMA_COMPATIBLE_PROVIDER_ID, OllamaCompatibleConfig,
    OllamaConfigError, OllamaProfile,
};
pub use disposition::{OllamaFailureDisposition, classify_ollama_failure};
pub use evaluator::OllamaCompatibleEvaluator;
pub use metadata::build_hosted_metadata_bundle;
pub use transport::OllamaCompatibleHttpTransport;
pub use workflow::HostedOllamaWorkflowEvaluator;
