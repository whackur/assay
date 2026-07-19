use assay_domain::{ContentHash, RevisionId};

use crate::cache::{
    digest::{length_prefixed, sha256_content_hash},
    version::{CacheVersion, ProviderRepositoryId},
};

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
