use std::{error::Error, fmt};

use crate::http::RateLimitState;

/// Failing collection stage, without repository data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStage {
    Metadata,
    Revision,
    Tree,
    /// Downstream streaming analysis sink.
    Sink,
}

impl fmt::Display for CollectionStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Metadata => "metadata",
            Self::Revision => "revision",
            Self::Tree => "tree",
            Self::Sink => "sink",
        };
        formatter.write_str(value)
    }
}

/// Stable collection error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionErrorKind {
    /// Outer transport failed before a response was available.
    Transport,
    /// GitHub returned an unexpected HTTP status.
    HttpStatus,
    NotFound,
    /// Public collection was requested for a private repository.
    NotPublic,
    RateLimited,
    /// Structured response data violated the provider contract.
    InvalidProviderResponse,
    /// Configured response byte bound was reached.
    ResponseLimit,
    /// Streaming consumer could not accept another item.
    Sink,
}

impl fmt::Display for CollectionErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Transport => "transport",
            Self::HttpStatus => "http_status",
            Self::NotFound => "not_found",
            Self::NotPublic => "not_public",
            Self::RateLimited => "rate_limited",
            Self::InvalidProviderResponse => "invalid_provider_response",
            Self::ResponseLimit => "response_limit",
            Self::Sink => "sink",
        };
        formatter.write_str(value)
    }
}

/// Non-sensitive GitHub collection failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionError {
    kind: CollectionErrorKind,
    stage: CollectionStage,
    rate_limit: Option<RateLimitState>,
}

impl CollectionError {
    pub(crate) const fn new(kind: CollectionErrorKind, stage: CollectionStage) -> Self {
        Self {
            kind,
            stage,
            rate_limit: None,
        }
    }

    pub(crate) fn rate_limited(stage: CollectionStage, state: RateLimitState) -> Self {
        Self {
            kind: CollectionErrorKind::RateLimited,
            stage,
            rate_limit: Some(state),
        }
    }

    pub const fn kind(&self) -> CollectionErrorKind {
        self.kind
    }

    pub const fn stage(&self) -> CollectionStage {
        self.stage
    }

    /// Explicit rate-limit state for rate-limit failures.
    pub const fn rate_limit(&self) -> Option<&RateLimitState> {
        self.rate_limit.as_ref()
    }
}

impl fmt::Display for CollectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "GitHub collection failed during {}: {}",
            self.stage, self.kind
        )
    }
}

impl Error for CollectionError {}