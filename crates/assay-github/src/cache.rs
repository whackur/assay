use std::{error::Error, fmt, str::FromStr};

use assay_domain::{ContentHash, RevisionId, RuleSetHash};
use sha2::{Digest, Sha256};

const MAX_VERSION_BYTES: usize = 100;

/// A non-sensitive cache contract validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheValueError {
    kind: &'static str,
    reason: &'static str,
}

impl CacheValueError {
    const fn new(kind: &'static str, reason: &'static str) -> Self {
        Self { kind, reason }
    }

    /// Returns the rejected value kind without returning the value.
    pub const fn kind(self) -> &'static str {
        self.kind
    }

    /// Returns a non-sensitive validation reason.
    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for CacheValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}: {}", self.kind, self.reason)
    }
}

impl Error for CacheValueError {}

/// GitHub's stable numeric repository identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderRepositoryId(u64);

impl ProviderRepositoryId {
    /// Creates a non-zero provider repository identifier.
    pub fn new(value: u64) -> Result<Self, CacheValueError> {
        if value == 0 {
            return Err(CacheValueError::new(
                "provider_repository_id",
                "zero is not a provider repository identifier",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the numeric provider repository identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A canonical version or evaluator profile component used in cache keys.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheVersion(String);

impl CacheVersion {
    /// Parses a lowercase portable cache-key component.
    pub fn parse(value: &str) -> Result<Self, CacheValueError> {
        if value.is_empty()
            || value.len() > MAX_VERSION_BYTES
            || value.contains("..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
            || !value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !value
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Err(CacheValueError::new(
                "cache_version",
                "expected a canonical lowercase version component",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the canonical value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated GitHub Git object identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GitHubObjectId(String);

impl GitHubObjectId {
    /// Returns the lowercase full object identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for GitHubObjectId {
    type Err = CacheValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !matches!(value.len(), 40 | 64)
            || value.bytes().all(|byte| byte == b'0')
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(CacheValueError::new(
                "github_object_id",
                "expected a full lowercase non-null Git object identifier",
            ));
        }
        Ok(Self(value.to_owned()))
    }
}

/// The complete content identity of an equivalent project evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationKey {
    repository_id: ProviderRepositoryId,
    revision: RevisionId,
    evidence_version: CacheVersion,
    evaluation_version: CacheVersion,
    rubric_version: CacheVersion,
    evaluator_profile: CacheVersion,
    canonical_material: String,
    digest: ContentHash,
}

impl EvaluationKey {
    /// Constructs an account-independent, fully versioned evaluation key.
    pub fn new(
        repository_id: ProviderRepositoryId,
        revision: RevisionId,
        evidence_version: CacheVersion,
        evaluation_version: CacheVersion,
        rubric_version: CacheVersion,
        evaluator_profile: CacheVersion,
    ) -> Self {
        let components = [
            "github".to_owned(),
            repository_id.get().to_string(),
            revision.as_str().to_owned(),
            evidence_version.as_str().to_owned(),
            evaluation_version.as_str().to_owned(),
            rubric_version.as_str().to_owned(),
            evaluator_profile.as_str().to_owned(),
        ];
        let canonical_material = length_prefixed(&components);
        let digest = sha256_content_hash(canonical_material.as_bytes());
        Self {
            repository_id,
            revision,
            evidence_version,
            evaluation_version,
            rubric_version,
            evaluator_profile,
            canonical_material,
            digest,
        }
    }

    /// Returns the provider repository identifier.
    pub const fn repository_id(&self) -> ProviderRepositoryId {
        self.repository_id
    }

    /// Returns the immutable source revision.
    pub const fn revision(&self) -> &RevisionId {
        &self.revision
    }

    /// Returns the evidence extractor version.
    pub const fn evidence_version(&self) -> &CacheVersion {
        &self.evidence_version
    }

    /// Returns the evaluation compiler version.
    pub const fn evaluation_version(&self) -> &CacheVersion {
        &self.evaluation_version
    }

    /// Returns the rubric version.
    pub const fn rubric_version(&self) -> &CacheVersion {
        &self.rubric_version
    }

    /// Returns the evaluator profile.
    pub const fn evaluator_profile(&self) -> &CacheVersion {
        &self.evaluator_profile
    }

    /// Returns canonical length-prefixed material used to derive the digest.
    pub fn canonical_material(&self) -> &str {
        &self.canonical_material
    }

    /// Returns the stable SHA-256 key digest.
    pub const fn digest(&self) -> &ContentHash {
        &self.digest
    }
}

/// A lookup result from an outer evaluation cache adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationCacheState {
    /// An immutable equivalent result is reusable.
    Hit,
    /// An equivalent evaluation is already running.
    InFlight,
    /// No equivalent evaluation is known.
    Miss,
    /// Cache state could not be established and must not be treated as a miss.
    Unavailable,
}

/// Read-only lookup boundary for persistent or in-memory evaluation caches.
pub trait EvaluationCacheLookup {
    /// Looks up the complete versioned evaluation key.
    fn lookup(&self, key: &EvaluationKey) -> EvaluationCacheState;
}

/// A cache-aware evaluation admission plan without quota side effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationReuse {
    /// Reuse the immutable result.
    Hit,
    /// Join the equivalent in-flight evaluation.
    InFlight,
    /// A new evaluation is required.
    Miss,
    /// Cache availability is unknown; callers must preserve this state.
    Unavailable,
}

/// Maps a cache lookup into an explicit reuse decision.
pub fn plan_evaluation(cache: &impl EvaluationCacheLookup, key: &EvaluationKey) -> EvaluationReuse {
    match cache.lookup(key) {
        EvaluationCacheState::Hit => EvaluationReuse::Hit,
        EvaluationCacheState::InFlight => EvaluationReuse::InFlight,
        EvaluationCacheState::Miss => EvaluationReuse::Miss,
        EvaluationCacheState::Unavailable => EvaluationReuse::Unavailable,
    }
}

/// The content identity for one versioned blob analysis.
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

    /// Returns the immutable blob object identifier.
    pub const fn blob(&self) -> &GitHubObjectId {
        &self.blob
    }

    /// Returns the analyzer version.
    pub const fn analyzer_version(&self) -> &CacheVersion {
        &self.analyzer_version
    }

    /// Returns the rule-set hash.
    pub const fn rule_set_hash(&self) -> &RuleSetHash {
        &self.rule_set_hash
    }

    /// Returns the stable SHA-256 cache digest.
    pub const fn digest(&self) -> &ContentHash {
        &self.digest
    }
}

/// A read-only blob-analysis cache result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobCacheState {
    /// The matching blob analysis can be reused.
    Hit,
    /// The matching blob requires analysis.
    Miss,
    /// Cache availability is unknown; analysis may proceed without reuse.
    Unavailable,
}

/// Read-only lookup boundary for blob-hash incremental analysis.
pub trait BlobCacheLookup {
    /// Looks up a blob by object hash, analyzer version, and rules.
    fn lookup(&self, key: &BlobAnalysisKey) -> BlobCacheState;
}

fn length_prefixed(components: &[String]) -> String {
    let mut material = String::new();
    for component in components {
        use std::fmt::Write as _;
        write!(&mut material, "{}:{}|", component.len(), component)
            .expect("writing to a String cannot fail");
    }
    material
}

fn sha256_content_hash(bytes: &[u8]) -> ContentHash {
    let digest = Sha256::digest(bytes);
    ContentHash::from_str(&format!("sha256:{digest:x}"))
        .expect("SHA-256 always produces a valid domain content hash")
}
