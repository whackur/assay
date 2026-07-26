use std::time::Duration;

use crate::{EvaluationErrorKind, ProviderRequest};

use super::provenance::{ProviderReply, SamplingConfig};
use super::secret::SecretName;
use super::transport::AuthorizationScheme;

/// Per-provider parts of an API-key adapter: identity, envelope building,
/// authorization form, status classification, and response extraction.
/// Implementations hold their own `Config` and never perform I/O; the shared
/// [`ApiKeyEvaluator`] drives the two injected ports and the one validator.
pub trait ApiProviderProfile {
    fn provider_id(&self) -> &'static str;

    fn endpoint(&self) -> &str;

    fn model(&self) -> &str;

    /// Reference name of the provider credential, or none for an unauthenticated compatible endpoint.
    fn secret_name(&self) -> Option<&SecretName>;

    fn sampling(&self) -> SamplingConfig;

    fn timeout(&self) -> Duration;

    /// Authorization header form when a credential is configured; key material never appears.
    fn authorization(&self) -> Option<AuthorizationScheme>;

    /// Builds the provider request body from the canonical payload; never contains the credential.
    fn request_body(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, EvaluationErrorKind>;

    /// Classifies a non-success HTTP status into the shared failure taxonomy.
    fn classify_http_status(&self, status: u16) -> Option<EvaluationErrorKind>;

    /// Extracts the untrusted judgment text and telemetry from a response body.
    fn extract_reply(&self, body: &[u8]) -> Result<ProviderReply, EvaluationErrorKind>;
}