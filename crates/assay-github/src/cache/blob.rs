use assay_domain::{ContentHash, RuleSetHash};

use crate::cache::{
    digest::{length_prefixed, sha256_content_hash},
    version::{CacheVersion, GitHubObjectId},
};

/// Content identity for one versioned blob analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobAnalysisKey {
    blob: GitHubObjectId,
    analyzer_version: CacheVersion,
    rule_set_hash: RuleSetHash,
    digest: ContentHash,
}

impl BlobAnalysisKey {
    /// Creates a blob-analysis key that invalidates on analyzer or rule changes.
    pub fn new(
        blob: GitHubObjectId,
        analyzer_version: CacheVersion,
        rule_set_hash: RuleSetHash,
    ) -> Self {
        let components = [
            "github_blob".to_owned(),
            blob.as_str().to_owned(),
            analyzer_version.as_str().to_owned(),
            rule_set_hash.as_str().to_owned(),
        ];
        let digest = sha256_content_hash(length_prefixed(&components).as_bytes());
        Self {
            blob,
            analyzer_version,
            rule_set_hash,
            digest,
        }
    }

    pub const fn blob(&self) -> &GitHubObjectId {
        &self.blob
    }

    pub const fn analyzer_version(&self) -> &CacheVersion {
        &self.analyzer_version
    }

    pub const fn rule_set_hash(&self) -> &RuleSetHash {
        &self.rule_set_hash
    }

    /// Stable SHA-256 cache digest.
    pub const fn digest(&self) -> &ContentHash {
        &self.digest
    }
}

/// Read-only blob-analysis cache result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobCacheState {
    /// Matching blob analysis can be reused.
    Hit,
    /// Matching blob requires analysis.
    Miss,
    /// Cache availability is unknown; analysis may proceed without reuse.
    Unavailable,
}

/// Read-only lookup boundary for blob-hash incremental analysis.
pub trait BlobCacheLookup {
    fn lookup(&self, key: &BlobAnalysisKey) -> BlobCacheState;
}