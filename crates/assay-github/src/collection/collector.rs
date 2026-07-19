use std::str::FromStr;

use assay_domain::{RevisionId, RuleSetHash};
use serde::Deserialize;

use crate::{
    BlobCacheLookup, CacheVersion, CanonicalGitHubRepository, GitHubHttp, GitHubRequest,
    GitHubResponse, ProviderRepositoryId, RateLimitState, TreeCollectionLimits,
    TreeCollectionSummary, TreeSink,
    collection::{
        error::{CollectionError, CollectionErrorKind, CollectionStage},
        reader::{parse_json_limited, percent_encode_path_segment},
        source::{GitHubRepositoryMetadata, ResolvedGitHubSource, RevisionSelector},
    },
    http::rate_limit_state,
    tree::deserialize_tree_response,
};

const METADATA_RESPONSE_LIMIT: usize = 64 * 1024;
const REVISION_RESPONSE_LIMIT: usize = 512 * 1024;

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
