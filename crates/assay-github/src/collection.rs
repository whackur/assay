use std::{error::Error, fmt, io, io::Read, str::FromStr};

use assay_domain::{RevisionId, RuleSetHash};
use serde::Deserialize;

use crate::{
    BlobCacheLookup, CacheVersion, CanonicalGitHubRepository, GitHubHttp, GitHubRequest,
    GitHubResponse, ProviderRepositoryId, RateLimitState, RepositoryInputError,
    TreeCollectionLimits, TreeCollectionSummary, TreeSink, http::rate_limit_state,
    tree::deserialize_tree_response,
};

const METADATA_RESPONSE_LIMIT: usize = 64 * 1024;
const REVISION_RESPONSE_LIMIT: usize = 16 * 1024;
const LIMIT_MARKER: &str = "github_response_limit_exceeded";

/// A user-selected revision that must resolve to a full immutable object ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionSelector {
    /// Resolve the repository's reported default branch.
    DefaultBranch,
    /// Resolve a named branch, tag, or full object identifier.
    Named(String),
}

impl RevisionSelector {
    /// Creates a bounded ref selector. Its value is percent-encoded in requests.
    pub fn named(value: &str) -> Result<Self, RepositoryInputError> {
        if value.is_empty()
            || value.len() > 255
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(RepositoryInputError::revision_selector());
        }
        Ok(Self::Named(value.to_owned()))
    }
}

/// A GitHub source pinned to a stable provider ID and immutable revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGitHubSource {
    repository_id: ProviderRepositoryId,
    repository: CanonicalGitHubRepository,
    revision: RevisionId,
    selected_ref: String,
    rate_limit: RateLimitState,
    metadata: GitHubRepositoryMetadata,
    metadata_etag: Option<String>,
}

impl ResolvedGitHubSource {
    /// Returns GitHub's stable numeric repository identifier.
    pub const fn repository_id(&self) -> ProviderRepositoryId {
        self.repository_id
    }

    /// Returns the provider-confirmed canonical repository.
    pub const fn repository(&self) -> &CanonicalGitHubRepository {
        &self.repository
    }

    /// Returns the full immutable commit identifier.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }

    /// Returns the ref used for immutable resolution.
    pub fn selected_ref(&self) -> &str {
        &self.selected_ref
    }

    /// Returns rate-limit state from the revision response.
    pub const fn rate_limit(&self) -> &RateLimitState {
        &self.rate_limit
    }

    /// Returns the bounded normalized public metadata projection.
    pub const fn metadata(&self) -> &GitHubRepositoryMetadata {
        &self.metadata
    }

    /// Returns the metadata response ETag when GitHub supplied one.
    pub fn metadata_etag(&self) -> Option<&str> {
        self.metadata_etag.as_deref()
    }
}

/// Bounded public metadata used by hosted project collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRepositoryMetadata {
    pub description: Option<String>,
    pub stargazers_count: u64,
    pub forks_count: u64,
    pub open_issues_count: u64,
    pub archived: bool,
    pub fork: bool,
    pub license_spdx: Option<String>,
}

/// The failing collection stage, without repository data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStage {
    /// Repository metadata lookup.
    Metadata,
    /// Immutable revision resolution.
    Revision,
    /// Recursive Git tree collection.
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

/// A stable collection error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionErrorKind {
    /// Outer transport failed before a response was available.
    Transport,
    /// GitHub returned an unexpected HTTP status.
    HttpStatus,
    /// The repository or revision was not found.
    NotFound,
    /// Public collection was requested for a private repository.
    NotPublic,
    /// GitHub rate limiting prevented collection.
    RateLimited,
    /// Structured response data violated the provider contract.
    InvalidProviderResponse,
    /// A configured response byte bound was reached.
    ResponseLimit,
    /// The streaming consumer could not accept another item.
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

/// A non-sensitive GitHub collection failure.
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

    /// Returns the stable error category.
    pub const fn kind(&self) -> CollectionErrorKind {
        self.kind
    }

    /// Returns the failed stage.
    pub const fn stage(&self) -> CollectionStage {
        self.stage
    }

    /// Returns explicit rate-limit state for rate-limit failures.
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

/// Read-only GitHub collector over an injected fixed-origin HTTP adapter.
pub struct GitHubCollector<'a, H> {
    http: &'a mut H,
}

impl<'a, H: GitHubHttp> GitHubCollector<'a, H> {
    /// Creates a collector without performing network I/O.
    pub fn new(http: &'a mut H) -> Self {
        Self { http }
    }

    /// Resolves repository metadata and a selected ref to an immutable commit.
    pub fn resolve_revision(
        &mut self,
        repository: &CanonicalGitHubRepository,
        selector: RevisionSelector,
    ) -> Result<ResolvedGitHubSource, CollectionError> {
        let metadata_path = format!("/repos/{}/{}", repository.owner(), repository.name());
        let (metadata_response, _) =
            self.request(GitHubRequest::get(metadata_path), CollectionStage::Metadata)?;
        let metadata_etag = metadata_response.header("etag").map(str::to_owned);
        let metadata: RepositoryMetadata = parse_json_limited(
            metadata_response,
            METADATA_RESPONSE_LIMIT,
            CollectionStage::Metadata,
        )?;
        if metadata.private {
            return Err(CollectionError::new(
                CollectionErrorKind::NotPublic,
                CollectionStage::Metadata,
            ));
        }
        let repository_id = ProviderRepositoryId::new(metadata.id).map_err(|_| {
            CollectionError::new(
                CollectionErrorKind::InvalidProviderResponse,
                CollectionStage::Metadata,
            )
        })?;
        let provider_repository = CanonicalGitHubRepository::parse(&format!(
            "{}/{}",
            metadata.owner.login, metadata.name
        ))
        .map_err(|_| {
            CollectionError::new(
                CollectionErrorKind::InvalidProviderResponse,
                CollectionStage::Metadata,
            )
        })?;
        let selected_ref = match selector {
            RevisionSelector::DefaultBranch => validate_provider_ref(metadata.default_branch)?,
            RevisionSelector::Named(value) => value,
        };
        let encoded_ref = percent_encode_path_segment(selected_ref.as_bytes());
        let revision_path = format!(
            "/repos/{}/{}/commits/{encoded_ref}",
            provider_repository.owner(),
            provider_repository.name()
        );
        let (revision_response, rate_limit) =
            self.request(GitHubRequest::get(revision_path), CollectionStage::Revision)?;
        let commit: CommitResponse = parse_json_limited(
            revision_response,
            REVISION_RESPONSE_LIMIT,
            CollectionStage::Revision,
        )?;
        let revision = RevisionId::from_str(&commit.sha).map_err(|_| {
            CollectionError::new(
                CollectionErrorKind::InvalidProviderResponse,
                CollectionStage::Revision,
            )
        })?;
        Ok(ResolvedGitHubSource {
            repository_id,
            repository: provider_repository,
            revision,
            selected_ref,
            rate_limit,
            metadata: GitHubRepositoryMetadata {
                description: metadata.description,
                stargazers_count: metadata.stargazers_count,
                forks_count: metadata.forks_count,
                open_issues_count: metadata.open_issues_count,
                archived: metadata.archived,
                fork: metadata.fork,
                license_spdx: metadata.license.and_then(|license| license.spdx_id),
            },
            metadata_etag,
        })
    }

    /// Streams bounded blob work for an immutable recursive tree.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_tree<C: BlobCacheLookup, S: TreeSink>(
        &mut self,
        repository: &CanonicalGitHubRepository,
        revision: &RevisionId,
        cache: &C,
        analyzer_version: CacheVersion,
        rule_set_hash: RuleSetHash,
        limits: TreeCollectionLimits,
        sink: &mut S,
    ) -> Result<TreeCollectionSummary, CollectionError> {
        let path = format!(
            "/repos/{}/{}/git/trees/{}?recursive=1",
            repository.owner(),
            repository.name(),
            revision.as_str()
        );
        let (response, rate_limit) =
            self.request(GitHubRequest::get(path), CollectionStage::Tree)?;
        deserialize_tree_response(
            response,
            rate_limit,
            cache,
            analyzer_version,
            rule_set_hash,
            limits,
            sink,
        )
    }

    fn request(
        &mut self,
        request: GitHubRequest,
        stage: CollectionStage,
    ) -> Result<(GitHubResponse, RateLimitState), CollectionError> {
        let response = self
            .http
            .execute(request)
            .map_err(|_| CollectionError::new(CollectionErrorKind::Transport, stage))?;
        let rate_limit = rate_limit_state(&response);
        match response.status() {
            200 => Ok((response, rate_limit)),
            404 => Err(CollectionError::new(CollectionErrorKind::NotFound, stage)),
            429 => Err(CollectionError::rate_limited(stage, rate_limit)),
            403 if matches!(
                rate_limit,
                RateLimitState::Exhausted { .. } | RateLimitState::SecondaryLimited { .. }
            ) =>
            {
                Err(CollectionError::rate_limited(stage, rate_limit))
            }
            _ => Err(CollectionError::new(CollectionErrorKind::HttpStatus, stage)),
        }
    }
}

#[derive(Deserialize)]
struct RepositoryMetadata {
    id: u64,
    name: String,
    owner: RepositoryOwner,
    default_branch: String,
    private: bool,
    description: Option<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    forks_count: u64,
    #[serde(default)]
    open_issues_count: u64,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    fork: bool,
    license: Option<RepositoryLicense>,
}

#[derive(Deserialize)]
struct RepositoryLicense {
    spdx_id: Option<String>,
}

#[derive(Deserialize)]
struct RepositoryOwner {
    login: String,
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

fn validate_provider_ref(value: String) -> Result<String, CollectionError> {
    RevisionSelector::named(&value).map(|_| value).map_err(|_| {
        CollectionError::new(
            CollectionErrorKind::InvalidProviderResponse,
            CollectionStage::Metadata,
        )
    })
}

fn percent_encode_path_segment(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    output
}

pub(crate) struct LimitedReader<R> {
    inner: R,
    remaining: usize,
}

impl<R> LimitedReader<R> {
    pub(crate) const fn new(inner: R, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::other(LIMIT_MARKER)),
            };
        }
        let permitted = self.remaining.min(buffer.len());
        let read = self.inner.read(&mut buffer[..permitted])?;
        self.remaining -= read;
        Ok(read)
    }
}

fn parse_json_limited<T: for<'de> Deserialize<'de>>(
    response: GitHubResponse,
    limit: usize,
    stage: CollectionStage,
) -> Result<T, CollectionError> {
    if content_length_exceeds(&response, limit) {
        return Err(CollectionError::new(
            CollectionErrorKind::ResponseLimit,
            stage,
        ));
    }
    let reader = LimitedReader::new(response.into_body(), limit);
    serde_json::from_reader(reader).map_err(|error| {
        let kind = if error.to_string().contains(LIMIT_MARKER) {
            CollectionErrorKind::ResponseLimit
        } else {
            CollectionErrorKind::InvalidProviderResponse
        };
        CollectionError::new(kind, stage)
    })
}

pub(crate) fn content_length_exceeds(response: &GitHubResponse, limit: usize) -> bool {
    response
        .header("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > limit)
}

pub(crate) fn is_response_limit(error: &serde_json::Error) -> bool {
    error.to_string().contains(LIMIT_MARKER)
}
