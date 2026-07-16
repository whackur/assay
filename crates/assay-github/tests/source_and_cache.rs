use std::{collections::BTreeMap, str::FromStr};

use assay_domain::{RevisionId, RuleSetHash};
use assay_github::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, CacheVersion, CanonicalGitHubRepository,
    EvaluationCacheLookup, EvaluationCacheState, EvaluationKey, EvaluationReuse, GitHubObjectId,
    ProviderRepositoryId, plan_evaluation,
};

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const BLOB: &str = "89abcdef0123456789abcdef0123456789abcdef";
const RULES: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn canonicalizes_only_public_github_repository_inputs() {
    for input in [
        "Assay-Project/Assay",
        "https://github.com/Assay-Project/Assay",
        "https://github.com/Assay-Project/Assay.git/",
        "https://www.github.com/Assay-Project/Assay/",
    ] {
        let repository = CanonicalGitHubRepository::parse(input).unwrap();
        assert_eq!(repository.owner(), "assay-project");
        assert_eq!(repository.name(), "assay");
        assert_eq!(repository.identifier(), "assay-project/assay");
        assert_eq!(repository.url(), "https://github.com/assay-project/assay");
    }
}

#[test]
fn rejects_ambiguous_or_non_github_repository_inputs_without_echoing_them() {
    for input in [
        "http://github.com/owner/repo",
        "https://github.example/owner/repo",
        "https://user@github.com/owner/repo",
        "https://github.com:443/owner/repo",
        "https://github.com/owner/repo/issues",
        "https://github.com/owner/repo?token=secret",
        "https://github.com/owner/repo#fragment",
        "git@github.com:owner/repo.git",
        "owner/../repo",
        "/absolute/repo",
        "owner/repo/extra",
        "owner/.git",
        "owner/repo%2fother",
    ] {
        let error = CanonicalGitHubRepository::parse(input).unwrap_err();
        assert!(!error.to_string().contains(input));
        assert!(!error.to_string().contains("secret"));
    }
}

#[test]
fn evaluation_keys_are_versioned_content_keys_without_account_identity() {
    let repository_id = ProviderRepositoryId::new(42).unwrap();
    let revision = RevisionId::from_str(REVISION).unwrap();
    let base = EvaluationKey::new(
        repository_id,
        revision.clone(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("project-intelligence-1").unwrap(),
        CacheVersion::parse("project-rubric-1").unwrap(),
        CacheVersion::parse("anonymous-1").unwrap(),
    );
    let same = EvaluationKey::new(
        repository_id,
        revision,
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("project-intelligence-1").unwrap(),
        CacheVersion::parse("project-rubric-1").unwrap(),
        CacheVersion::parse("anonymous-1").unwrap(),
    );
    let changed_profile = EvaluationKey::new(
        repository_id,
        RevisionId::from_str(REVISION).unwrap(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("project-intelligence-1").unwrap(),
        CacheVersion::parse("project-rubric-1").unwrap(),
        CacheVersion::parse("authenticated-1").unwrap(),
    );

    assert_eq!(base.digest(), same.digest());
    assert_ne!(base.digest(), changed_profile.digest());
    assert!(base.digest().as_str().starts_with("sha256:"));
    assert!(!base.canonical_material().contains("account"));
}

#[test]
fn every_evaluation_version_dimension_changes_the_key() {
    let make = |evidence: &str, evaluation: &str, rubric: &str, profile: &str| {
        EvaluationKey::new(
            ProviderRepositoryId::new(7).unwrap(),
            RevisionId::from_str(REVISION).unwrap(),
            CacheVersion::parse(evidence).unwrap(),
            CacheVersion::parse(evaluation).unwrap(),
            CacheVersion::parse(rubric).unwrap(),
            CacheVersion::parse(profile).unwrap(),
        )
        .digest()
        .as_str()
        .to_owned()
    };
    let baseline = make("evidence-1", "evaluation-1", "rubric-1", "profile-1");
    for changed in [
        make("evidence-2", "evaluation-1", "rubric-1", "profile-1"),
        make("evidence-1", "evaluation-2", "rubric-1", "profile-1"),
        make("evidence-1", "evaluation-1", "rubric-2", "profile-1"),
        make("evidence-1", "evaluation-1", "rubric-1", "profile-2"),
    ] {
        assert_ne!(baseline, changed);
    }
}

struct FakeEvaluationCache(EvaluationCacheState);

impl EvaluationCacheLookup for FakeEvaluationCache {
    fn lookup(&self, _key: &EvaluationKey) -> EvaluationCacheState {
        self.0
    }
}

#[test]
fn evaluation_cache_states_preserve_hit_in_flight_miss_and_unavailable() {
    let key = EvaluationKey::new(
        ProviderRepositoryId::new(42).unwrap(),
        RevisionId::from_str(REVISION).unwrap(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("evaluation-1").unwrap(),
        CacheVersion::parse("rubric-1").unwrap(),
        CacheVersion::parse("profile-1").unwrap(),
    );
    for (state, expected) in [
        (EvaluationCacheState::Hit, EvaluationReuse::Hit),
        (EvaluationCacheState::InFlight, EvaluationReuse::InFlight),
        (EvaluationCacheState::Miss, EvaluationReuse::Miss),
        (
            EvaluationCacheState::Unavailable,
            EvaluationReuse::Unavailable,
        ),
    ] {
        assert_eq!(plan_evaluation(&FakeEvaluationCache(state), &key), expected);
    }
}

struct FakeBlobCache(BTreeMap<String, BlobCacheState>);

impl BlobCacheLookup for FakeBlobCache {
    fn lookup(&self, key: &BlobAnalysisKey) -> BlobCacheState {
        self.0
            .get(key.blob().as_str())
            .copied()
            .unwrap_or(BlobCacheState::Miss)
    }
}

#[test]
fn blob_cache_keys_include_blob_analyzer_and_rule_versions() {
    let blob = GitHubObjectId::from_str(BLOB).unwrap();
    let baseline = BlobAnalysisKey::new(
        blob.clone(),
        CacheVersion::parse("static-evidence-1").unwrap(),
        RuleSetHash::from_str(RULES).unwrap(),
    );
    let analyzer_changed = BlobAnalysisKey::new(
        blob.clone(),
        CacheVersion::parse("static-evidence-2").unwrap(),
        RuleSetHash::from_str(RULES).unwrap(),
    );
    let blob_changed = BlobAnalysisKey::new(
        GitHubObjectId::from_str(REVISION).unwrap(),
        CacheVersion::parse("static-evidence-1").unwrap(),
        RuleSetHash::from_str(RULES).unwrap(),
    );

    assert_ne!(baseline.digest(), analyzer_changed.digest());
    assert_ne!(baseline.digest(), blob_changed.digest());

    let cache = FakeBlobCache(BTreeMap::from([(BLOB.to_owned(), BlobCacheState::Hit)]));
    assert_eq!(cache.lookup(&baseline), BlobCacheState::Hit);
    assert_eq!(cache.lookup(&blob_changed), BlobCacheState::Miss);
}

#[test]
fn blob_cache_key_changes_when_the_rule_set_hash_changes() {
    const OTHER_RULES: &str =
        "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    let blob = GitHubObjectId::from_str(BLOB).unwrap();
    let baseline = BlobAnalysisKey::new(
        blob.clone(),
        CacheVersion::parse("static-evidence-1").unwrap(),
        RuleSetHash::from_str(RULES).unwrap(),
    );
    let rules_changed = BlobAnalysisKey::new(
        blob,
        CacheVersion::parse("static-evidence-1").unwrap(),
        RuleSetHash::from_str(OTHER_RULES).unwrap(),
    );
    assert_ne!(baseline.digest(), rules_changed.digest());
}

#[test]
fn cache_keys_are_domain_separated_and_resist_component_boundary_shifts() {
    let blob = GitHubObjectId::from_str(BLOB).unwrap();
    let blob_key = BlobAnalysisKey::new(
        blob,
        CacheVersion::parse("evidence-1").unwrap(),
        RuleSetHash::from_str(RULES).unwrap(),
    );
    let evaluation_key = EvaluationKey::new(
        ProviderRepositoryId::new(42).unwrap(),
        RevisionId::from_str(REVISION).unwrap(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("evaluation-1").unwrap(),
        CacheVersion::parse("rubric-1").unwrap(),
        CacheVersion::parse("profile-1").unwrap(),
    );
    assert_ne!(blob_key.digest(), evaluation_key.digest());

    let shifted_left = EvaluationKey::new(
        ProviderRepositoryId::new(42).unwrap(),
        RevisionId::from_str(REVISION).unwrap(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("evaluation-1").unwrap(),
        CacheVersion::parse("ab").unwrap(),
        CacheVersion::parse("c").unwrap(),
    );
    let shifted_right = EvaluationKey::new(
        ProviderRepositoryId::new(42).unwrap(),
        RevisionId::from_str(REVISION).unwrap(),
        CacheVersion::parse("evidence-1").unwrap(),
        CacheVersion::parse("evaluation-1").unwrap(),
        CacheVersion::parse("a").unwrap(),
        CacheVersion::parse("bc").unwrap(),
    );
    assert_ne!(shifted_left.digest(), shifted_right.digest());
}

#[test]
fn cache_versions_and_provider_ids_reject_ambiguous_values() {
    assert!(ProviderRepositoryId::new(0).is_err());
    for value in ["", "Profile-1", "../profile", "profile 1", "a//b"] {
        let error = CacheVersion::parse(value).unwrap_err();
        if !value.is_empty() {
            assert!(!error.to_string().contains(value));
        }
    }
}
