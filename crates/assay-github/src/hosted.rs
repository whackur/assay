use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assay_project_intelligence::{
    HostedClaimedJob, HostedFailure, HostedSourceCollection, HostedSourceCollectionPort,
};
use reqwest::{blocking::Client, header, redirect::Policy};
use serde_json::{Value, json};

use crate::{
    CanonicalGitHubRepository, CollectionErrorKind, GitHubCollector, GitHubHttp, GitHubRequest,
    GitHubResponse, RateLimitState, RevisionSelector, TransportError,
};

const API_ORIGIN: &str = "https://api.github.com";
const USER_AGENT: &str = "assay-github/0.1";

/// Normalized public GitHub facts ready for append-only hosted persistence.
#[derive(Clone, Debug)]
pub struct HostedGitHubCollection {
    pub provider_repository_id: i64,
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub default_branch: String,
    pub head_sha: String,
    pub source_url: String,
    pub etag: Option<String>,
    pub normalized_facts: Value,
}

/// Stable redacted collection failure classified at the GitHub boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedGitHubFailure {
    code: &'static str,
    retryable: bool,
    retry_after_seconds: Option<u64>,
}

impl HostedGitHubFailure {
    pub const fn code(self) -> &'static str {
        self.code
    }

    pub const fn retryable(self) -> bool {
        self.retryable
    }

    pub const fn retry_after_seconds(self) -> Option<u64> {
        self.retry_after_seconds
    }

    pub fn affects_provider_circuit(self) -> bool {
        !matches!(
            self.code,
            "github_repository_invalid" | "github_repository_unavailable"
        )
    }
}

/// Hosted projection over the existing injectable GitHub collection seam.
pub struct HostedGitHubAdapter<H> {
    http: H,
}

impl<H: GitHubHttp> HostedGitHubAdapter<H> {
    pub const fn new(http: H) -> Self {
        Self { http }
    }

    /// Returns the transport after collection for deterministic inspection.
    pub fn into_transport(self) -> H {
        self.http
    }

    pub fn collect(
        &mut self,
        owner: &str,
        repository: &str,
    ) -> Result<HostedGitHubCollection, HostedGitHubFailure> {
        let requested = CanonicalGitHubRepository::parse(&format!("{owner}/{repository}"))
            .map_err(|_| failure("github_repository_invalid", false))?;
        let resolved = GitHubCollector::new(&mut self.http)
            .resolve_revision(&requested, RevisionSelector::DefaultBranch)
            .map_err(|error| classify(&error))?;
        let provider_repository_id = i64::try_from(resolved.repository_id().get())
            .map_err(|_| failure("github_repository_id_out_of_range", false))?;
        let canonical = resolved.repository();
        let metadata = resolved.metadata();
        let default_branch = resolved.selected_ref().to_owned();
        let head_sha = resolved.revision().as_str().to_owned();
        let normalized_facts = json!({
            "description": metadata.description,
            "stargazers_count": metadata.stargazers_count,
            "forks_count": metadata.forks_count,
            "open_issues_count": metadata.open_issues_count,
            "archived": metadata.archived,
            "fork": metadata.fork,
            "license_spdx": metadata.license_spdx,
            "default_branch": default_branch.clone(),
            "head_sha": head_sha.clone(),
            "full_name": canonical.identifier()
        });
        Ok(HostedGitHubCollection {
            provider_repository_id,
            owner: canonical.owner().to_owned(),
            repository: canonical.name().to_owned(),
            canonical_url: canonical.url().to_owned(),
            default_branch,
            head_sha,
            source_url: format!("{API_ORIGIN}/repos/{}", canonical.identifier()),
            etag: resolved.metadata_etag().map(str::to_owned),
            normalized_facts,
        })
    }
}

/// Production fixed-origin, no-redirect GitHub HTTP adapter.
pub struct ReqwestGitHubHttp {
    client: Client,
    token: Option<String>,
}

/// Production workflow adapter that keeps GitHub transport details out of the worker.
pub struct HostedGitHubWorkflowCollector {
    token: Option<String>,
}

impl HostedGitHubWorkflowCollector {
    pub const fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

impl HostedSourceCollectionPort for HostedGitHubWorkflowCollector {
    async fn collect(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<HostedSourceCollection, HostedFailure> {
        let token = self.token.clone();
        let owner = job.owner.clone();
        let repository = job.repository.clone();
        tokio::task::spawn_blocking(move || {
            let http = ReqwestGitHubHttp::new(token)
                .map_err(|_| HostedFailure::provider("github_adapter_failure", true))?;
            let mut adapter = HostedGitHubAdapter::new(http);
            let value = adapter.collect(&owner, &repository).map_err(|failure| {
                let mut mapped = if failure.affects_provider_circuit() {
                    HostedFailure::provider(failure.code(), failure.retryable())
                } else {
                    HostedFailure::new(failure.code(), failure.retryable())
                };
                mapped.retry_after_seconds = failure
                    .retry_after_seconds()
                    .and_then(|value| i64::try_from(value).ok());
                mapped
            })?;
            Ok(HostedSourceCollection {
                provider_repository_id: value.provider_repository_id,
                owner: value.owner,
                repository: value.repository,
                canonical_url: value.canonical_url,
                default_branch: value.default_branch,
                head_sha: value.head_sha,
                source_url: value.source_url,
                etag: value.etag,
                normalized_facts: value.normalized_facts,
            })
        })
        .await
        .map_err(|_| HostedFailure::provider("github_adapter_failure", true))?
    }
}

impl ReqwestGitHubHttp {
    pub fn new(token: Option<String>) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .redirect(Policy::none())
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self { client, token })
    }
}

impl GitHubHttp for ReqwestGitHubHttp {
    fn execute(&mut self, request: GitHubRequest) -> Result<GitHubResponse, TransportError> {
        let url = format!("{API_ORIGIN}{}", request.path());
        let mut outbound = self
            .client
            .get(url)
            .header(header::ACCEPT, "application/vnd.github+json");
        if let Some(token) = &self.token {
            outbound = outbound.bearer_auth(token);
        }
        let response = outbound
            .send()
            .map_err(|_| transport_error("github_network_failure"))?;
        let status = response.status().as_u16();
        let headers = [
            "etag",
            "retry-after",
            "x-ratelimit-limit",
            "x-ratelimit-remaining",
            "x-ratelimit-reset",
        ]
        .into_iter()
        .filter_map(|name| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| (name.to_owned(), value.to_owned()))
        })
        .collect();
        Ok(GitHubResponse::new(status, headers, Box::new(response)))
    }
}

fn classify(error: &crate::CollectionError) -> HostedGitHubFailure {
    let retry_after_seconds = error.rate_limit().and_then(rate_limit_delay_seconds);
    let mut classified = match error.kind() {
        CollectionErrorKind::Transport => failure("github_network_failure", true),
        CollectionErrorKind::RateLimited => failure("github_rate_limited", true),
        CollectionErrorKind::HttpStatus => failure("github_provider_failure", true),
        CollectionErrorKind::NotFound | CollectionErrorKind::NotPublic => {
            failure("github_repository_unavailable", false)
        }
        CollectionErrorKind::InvalidProviderResponse | CollectionErrorKind::ResponseLimit => {
            failure("github_invalid_provider_response", false)
        }
        CollectionErrorKind::Sink => failure("github_adapter_failure", false),
    };
    classified.retry_after_seconds = retry_after_seconds;
    classified
}

const fn failure(code: &'static str, retryable: bool) -> HostedGitHubFailure {
    HostedGitHubFailure {
        code,
        retryable,
        retry_after_seconds: None,
    }
}

fn rate_limit_delay_seconds(state: &RateLimitState) -> Option<u64> {
    match state {
        RateLimitState::Exhausted {
            reset_at_unix_seconds,
            retry_after_seconds,
            ..
        } => {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
            retry_after_seconds
                .as_ref()
                .copied()
                .max(reset_at_unix_seconds.map(|reset| reset.saturating_sub(now)))
        }
        RateLimitState::SecondaryLimited {
            retry_after_seconds,
        } => *retry_after_seconds,
        RateLimitState::Available { .. } | RateLimitState::Unknown => None,
    }
}

fn transport_error(code: &'static str) -> TransportError {
    TransportError::new(code).expect("hard-coded transport code is valid")
}
