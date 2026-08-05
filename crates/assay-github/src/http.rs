use std::{error::Error, fmt, io::Read};

/// One fixed-origin, read-only GitHub API request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRequest {
    path: String,
}

impl GitHubRequest {
    pub(crate) fn get(path: String) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Contract can only express a GET request.
    pub const fn is_read_only(&self) -> bool {
        true
    }

    /// Public collection never embeds an authorization value in the request.
    pub const fn authorization(&self) -> Option<&str> {
        None
    }
}

/// Streaming response returned by an outer GitHub HTTP adapter.
pub struct GitHubResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Box<dyn Read + Send>,
}

impl GitHubResponse {
    /// Creates a response without interpreting or logging its body.
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Box<dyn Read + Send>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub(crate) const fn status(&self) -> u16 {
        self.status
    }

    pub(crate) fn header(&self, expected: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(expected))
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn into_body(self) -> Box<dyn Read + Send> {
        self.body
    }
}

/// Transport failure containing only a stable non-sensitive code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportError {
    code: &'static str,
}

impl TransportError {
    pub fn new(code: &'static str) -> Result<Self, &'static str> {
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err("invalid transport error code");
        }
        Ok(Self { code })
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "GitHub transport failed: {}", self.code)
    }
}

impl Error for TransportError {}

/// Injectable fixed-origin HTTP boundary used by deterministic fake clients.
pub trait GitHubHttp {
    /// Implementations must pin the origin to `https://api.github.com` and must
    /// not log response bodies or credentials.
    fn execute(&mut self, request: GitHubRequest) -> Result<GitHubResponse, TransportError>;
}

/// Explicit GitHub API budget state captured from response headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateLimitState {
    Available {
        limit: u64,
        remaining: u64,
        /// GitHub's Unix reset timestamp.
        reset_at_unix_seconds: u64,
    },
    Exhausted {
        limit: Option<u64>,
        reset_at_unix_seconds: Option<u64>,
        retry_after_seconds: Option<u64>,
    },
    /// GitHub applied a secondary or abuse limit.
    SecondaryLimited { retry_after_seconds: Option<u64> },
    /// Rate headers missing or invalid; this is not unlimited capacity.
    Unknown,
}

pub(crate) fn rate_limit_state(response: &GitHubResponse) -> RateLimitState {
    let limit = response.header("x-ratelimit-limit").and_then(parse_u64);
    let remaining = response.header("x-ratelimit-remaining").and_then(parse_u64);
    let reset = response.header("x-ratelimit-reset").and_then(parse_u64);
    let retry_after = response.header("retry-after").and_then(parse_u64);

    if response.status() == 429 || (response.status() == 403 && retry_after.is_some()) {
        return RateLimitState::SecondaryLimited {
            retry_after_seconds: retry_after,
        };
    }
    if remaining == Some(0) {
        return RateLimitState::Exhausted {
            limit,
            reset_at_unix_seconds: reset,
            retry_after_seconds: retry_after,
        };
    }
    match (limit, remaining, reset) {
        (Some(limit), Some(remaining), Some(reset_at_unix_seconds)) => RateLimitState::Available {
            limit,
            remaining,
            reset_at_unix_seconds,
        },
        _ => RateLimitState::Unknown,
    }
}

fn parse_u64(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}
