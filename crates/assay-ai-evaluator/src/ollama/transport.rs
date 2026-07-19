use std::{
    io::Read,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::{blocking::Client, header};

use crate::api::{HttpTransport, OutboundRequest, TransportError, TransportResponse};

use super::config::MAX_RESPONSE_BYTES_EXPOSED as MAX_RESPONSE_BYTES;

/// Concrete bounded HTTP transport owned by the provider adapter crate.
pub struct OllamaCompatibleHttpTransport {
    client: Client,
}

impl OllamaCompatibleHttpTransport {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
}

impl HttpTransport for OllamaCompatibleHttpTransport {
    fn send(&self, request: &OutboundRequest) -> Result<TransportResponse, TransportError> {
        let started = Instant::now();
        let mut outbound = self
            .client
            .post(request.endpoint())
            .timeout(request.timeout())
            .header(header::CONTENT_TYPE, "application/json")
            .body(request.body().to_vec());
        if let (Some(name), Some(value)) =
            (request.authorization_header_name(), request.authorization())
        {
            outbound = outbound.header(name, value);
        }
        let mut response = outbound.send().map_err(|error| {
            if error.is_timeout() {
                TransportError::Timeout
            } else {
                TransportError::Network
            }
        })?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(TransportError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let retry_after = provider_retry_delay(response.headers());
        let mut body = Vec::new();
        response
            .by_ref()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut body)
            .map_err(|_| TransportError::Network)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(TransportError::ResponseTooLarge);
        }
        Ok(TransportResponse::new(status, body, started.elapsed()).with_retry_after(retry_after))
    }
}

fn provider_retry_delay(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let retry_after = headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let reset_delay = headers
        .get("x-ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .and_then(|reset| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
            Some(reset.saturating_sub(now))
        });
    retry_after.max(reset_delay).map(Duration::from_secs)
}
