use std::{fmt, time::Duration};

use super::secret::ProviderSecret;

/// Provider-specific authorization header form, without the credential.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorizationScheme {
    pub header_name: &'static str,
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
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Body bytes, which never contain the credential.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub const fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Header name carrying the credential.
    pub const fn authorization_header_name(&self) -> Option<&'static str> {
        self.header_name
    }

    /// Authorization header value; the only credential exposure.
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

/// Completed transport response. Status and body are untrusted.
pub struct TransportResponse {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
    pub(crate) latency: Duration,
    pub(crate) retry_after: Option<Duration>,
}

impl TransportResponse {
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