//! Read-only GitHub collection contracts for Assay.
//!
//! The crate canonicalizes public GitHub repository inputs, resolves refs to
//! immutable object identifiers, reports API budget state, and streams
//! bounded tree facts to analysis consumers. It never executes repository
//! code and does not persist credentials, source bodies, raw diffs, or cache
//! values. Persistent cache and HTTP implementations belong in outer
//! adapters; the traits here are deterministic seams for those adapters.

#![forbid(unsafe_code)]

mod cache;
mod collection;
mod http;
mod source;
mod tree;

pub use cache::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, CacheValueError, CacheVersion,
    EvaluationCacheLookup, EvaluationCacheState, EvaluationKey, EvaluationReuse, GitHubObjectId,
    ProviderRepositoryId, plan_evaluation,
};
pub use collection::{
    CollectionError, CollectionErrorKind, CollectionStage, GitHubCollector, ResolvedGitHubSource,
    RevisionSelector,
};
pub use http::{GitHubHttp, GitHubRequest, GitHubResponse, RateLimitState, TransportError};
pub use source::{CanonicalGitHubRepository, RepositoryInputError};
pub use tree::{
    BlobWorkItem, CollectionStatus, TreeCollectionLimits, TreeCollectionSummary, TreePartialReason,
    TreeSink, TreeSinkError,
};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
