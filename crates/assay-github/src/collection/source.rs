use assay_domain::RevisionId;

use crate::{
    cache::ProviderRepositoryId,
    http::RateLimitState,
    source::{CanonicalGitHubRepository, RepositoryInputError},
};

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
    pub(crate) repository_id: ProviderRepositoryId,
    pub(crate) repository: CanonicalGitHubRepository,
    pub(crate) revision: RevisionId,
    pub(crate) selected_ref: String,
    pub(crate) rate_limit: RateLimitState,
    pub(crate) metadata: GitHubRepositoryMetadata,
    pub(crate) metadata_etag: Option<String>,
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
