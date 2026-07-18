//! Read-only GitHub collection contracts for Assay.
//!
//! The crate canonicalizes public GitHub repository inputs, resolves refs to
//! immutable object identifiers, reports API budget state, and streams
//! bounded tree facts to analysis consumers. It never executes repository
//! code and does not persist credentials, source bodies, raw diffs, or cache
//! values. The hosted fixed-origin HTTP adapter lives here beside the
//! deterministic transport seams so application entrypoints remain thin.

#![forbid(unsafe_code)]

mod cache;
mod collection;
mod hosted;
mod http;
mod source;
mod tree;

pub use cache::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, CacheValueError, CacheVersion,
    EvaluationCacheLookup, EvaluationCacheState, EvaluationKey, EvaluationReuse, GitHubObjectId,
    ProviderRepositoryId, plan_evaluation,
};
pub use collection::{
    CollectionError, CollectionErrorKind, CollectionStage, GitHubCollector,
    GitHubRepositoryMetadata, ResolvedGitHubSource, RevisionSelector,
};
pub use hosted::{
    HostedGitHubAdapter, HostedGitHubCollection, HostedGitHubFailure,
    HostedGitHubWorkflowCollector, ReqwestGitHubHttp,
};
pub use http::{GitHubHttp, GitHubRequest, GitHubResponse, RateLimitState, TransportError};
pub use source::{CanonicalGitHubRepository, RepositoryInputError};
pub use tree::{
    BlobWorkItem, CollectionStatus, TreeCollectionLimits, TreeCollectionSummary, TreePartialReason,
    TreeSink, TreeSinkError,
};

/// Stable package identifier for diagnostics and capability reporting.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
