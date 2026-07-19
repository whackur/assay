use std::time::Duration;

use crate::{EvaluationErrorKind, ProviderRequest};

use super::provenance::{ProviderReply, SamplingConfig};
use super::secret::SecretName;
use super::transport::AuthorizationScheme;

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

    /// Returns the reference name of the provider credential, or none for an
    /// unauthenticated compatible endpoint.
    fn secret_name(&self) -> Option<&SecretName>;

    /// Returns the deterministic sampling settings recorded as provenance.
    fn sampling(&self) -> SamplingConfig;

    /// Returns the transport timeout budget.
    fn timeout(&self) -> Duration;

    /// Returns the authorization header form when a credential is configured;
    /// the key material never appears.
    fn authorization(&self) -> Option<AuthorizationScheme>;

    /// Builds the provider request body from the canonical payload. The body
    /// never contains the credential.
    fn request_body(&self, request: &ProviderRequest<'_>) -> Result<Vec<u8>, EvaluationErrorKind>;

    /// Classifies a non-success HTTP status into the shared failure taxonomy.
    fn classify_http_status(&self, status: u16) -> Option<EvaluationErrorKind>;

    /// Extracts the untrusted judgment text and telemetry from a response body.
    fn extract_reply(&self, body: &[u8]) -> Result<ProviderReply, EvaluationErrorKind>;
}
