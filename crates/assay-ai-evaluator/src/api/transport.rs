use std::{fmt, time::Duration};

use super::secret::ProviderSecret;

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
    pub(crate) endpoint: String,
    pub(crate) body: Vec<u8>,
    pub(crate) timeout: Duration,
    pub(crate) header_name: Option<&'static str>,
    pub(crate) authorization: Option<ProviderSecret>,
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
    pub const fn authorization_header_name(&self) -> Option<&'static str> {
        self.header_name
    }

    /// Returns the authorization header value; the only credential exposure.
    pub fn authorization(&self) -> Option<String> {
        self.authorization
            .as_ref()
            .map(|value| value.expose().to_owned())
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
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
    pub(crate) latency: Duration,
    pub(crate) retry_after: Option<Duration>,
}

impl TransportResponse {
    /// Builds a response from an observed status, body, and measured latency.
    pub fn new(status: u16, body: Vec<u8>, latency: Duration) -> Self {
        Self {
            status,
            body,
            latency,
            retry_after: None,
        }
    }

    /// Records the larger Retry-After/reset delay extracted by a bounded transport.
    pub fn with_retry_after(mut self, retry_after: Option<Duration>) -> Self {
        self.retry_after = retry_after;
        self
    }
}

impl fmt::Debug for TransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportResponse")
            .field("status", &self.status)
            .field("body_len", &self.body.len())
            .field("latency", &self.latency)
            .field("retry_after", &self.retry_after)
            .finish()
    }
}

/// Redacted transport failure with no request or response text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    Timeout,
    Network,
    ResponseTooLarge,
}

/// HTTP transport seam. Profiles may use an injected transport or a bounded
/// concrete transport owned beside the profile.
pub trait HttpTransport {
    /// Sends one outbound request and returns an untrusted response.
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError>;
}
