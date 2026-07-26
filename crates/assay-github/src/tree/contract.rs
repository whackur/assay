use std::{error::Error, fmt};

use crate::{BlobCacheState, GitHubObjectId, RateLimitState};

/// One uncached or cache-unavailable blob sent to a downstream analyzer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobWorkItem {
    pub(crate) path: String,
    pub(crate) blob: GitHubObjectId,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) cache_state: BlobCacheState,
}

impl BlobWorkItem {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn blob(&self) -> &GitHubObjectId {
        &self.blob
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn cache_state(&self) -> BlobCacheState {
        self.cache_state
    }
}

/// Downstream streaming failure with no path or source content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeSinkError {
    code: &'static str,
}

impl TreeSinkError {
    pub fn new(code: &'static str) -> Result<Self, &'static str> {
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err("invalid tree sink error code");
        }
        Ok(Self { code })
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for TreeSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "tree sink failed: {}", self.code)
    }
}

impl Error for TreeSinkError {}

/// Streaming consumer for blob analyses that cannot be reused from cache.
pub trait TreeSink {
    fn accept(&mut self, item: BlobWorkItem) -> Result<(), TreeSinkError>;
}

/// Bounded collection facts. Counts are observations, not quality scores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeCollectionSummary {
    pub(crate) status: super::limits::CollectionStatus,
    pub(crate) observed_entries: usize,
    pub(crate) observed_blobs: usize,
    pub(crate) cache_hits: usize,
    pub(crate) cache_misses: usize,
    pub(crate) cache_unavailable: usize,
    pub(crate) project_boundaries: Vec<String>,
    pub(crate) partial_reasons: Vec<super::limits::TreePartialReason>,
    pub(crate) rate_limit: RateLimitState,
}

impl TreeCollectionSummary {
    pub const fn status(&self) -> super::limits::CollectionStatus {
        self.status
    }

    pub const fn observed_entries(&self) -> usize {
        self.observed_entries
    }

    /// Blobs processed in detail within the local entry limit.
    pub const fn observed_blobs(&self) -> usize {
        self.observed_blobs
    }

    pub const fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    pub const fn cache_misses(&self) -> usize {
        self.cache_misses
    }

    /// Lookups whose cache state was unavailable.
    pub const fn cache_unavailable(&self) -> usize {
        self.cache_unavailable
    }

    /// Sorted repository-relative project roots. `.` is repository root.
    pub fn project_boundaries(&self) -> &[String] {
        &self.project_boundaries
    }

    /// Explicit reasons for partial tree evidence.
    pub fn partial_reasons(&self) -> &[super::limits::TreePartialReason] {
        &self.partial_reasons
    }

    pub const fn rate_limit(&self) -> &RateLimitState {
        &self.rate_limit
    }
}