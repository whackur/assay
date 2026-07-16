use std::{collections::VecDeque, io::Cursor, str::FromStr};

use assay_domain::{RevisionId, RuleSetHash};
use assay_github::{
    BlobAnalysisKey, BlobCacheLookup, BlobCacheState, BlobWorkItem, CacheVersion,
    CanonicalGitHubRepository, CollectionErrorKind, CollectionStatus, GitHubCollector, GitHubHttp,
    GitHubRequest, GitHubResponse, RateLimitState, RevisionSelector, TreeCollectionLimits,
    TreePartialReason, TreeSink, TreeSinkError,
};

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const BLOB_A: &str = "89abcdef0123456789abcdef0123456789abcdef";
const BLOB_B: &str = "abcdef0123456789abcdef0123456789abcdef01";
const RULES: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct FakeHttp {
    responses: VecDeque<Result<GitHubResponse, assay_github::TransportError>>,
    requests: Vec<GitHubRequest>,
}

impl FakeHttp {
    fn new(responses: Vec<GitHubResponse>) -> Self {
        Self {
            responses: responses.into_iter().map(Ok).collect(),
            requests: Vec::new(),
        }
    }
}

impl GitHubHttp for FakeHttp {
    fn execute(
        &mut self,
        request: GitHubRequest,
    ) -> Result<GitHubResponse, assay_github::TransportError> {
        self.requests.push(request);
        self.responses.pop_front().expect("fixture response")
    }
}

fn response(status: u16, headers: &[(&str, &str)], body: &str) -> GitHubResponse {
    GitHubResponse::new(
        status,
        headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        Box::new(Cursor::new(body.as_bytes().to_vec())),
    )
}

fn rate_headers() -> [(&'static str, &'static str); 3] {
    [
        ("x-ratelimit-limit", "60"),
        ("x-ratelimit-remaining", "59"),
        ("x-ratelimit-reset", "2000000000"),
    ]
}

#[test]
fn resolves_default_branch_to_an_immutable_revision_with_explicit_rate_state() {
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"Assay","owner":{"login":"Assay-Project"},"default_branch":"main","private":false}"#,
        ),
        response(200, &rate_headers(), &format!(r#"{{"sha":"{REVISION}"}}"#)),
    ]);
    let repository = CanonicalGitHubRepository::parse("Assay-Project/Assay").unwrap();
    let resolved = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap();

    assert_eq!(resolved.repository_id().get(), 42);
    assert_eq!(resolved.repository().identifier(), "assay-project/assay");
    assert_eq!(resolved.revision().as_str(), REVISION);
    assert_eq!(resolved.selected_ref(), "main");
    assert_eq!(
        resolved.rate_limit(),
        &RateLimitState::Available {
            limit: 60,
            remaining: 59,
            reset_at_unix_seconds: 2_000_000_000,
        }
    );
    assert_eq!(http.requests.len(), 2);
    assert!(http.requests.iter().all(GitHubRequest::is_read_only));
    assert_eq!(http.requests[0].path(), "/repos/assay-project/assay");
    assert_eq!(
        http.requests[1].path(),
        "/repos/assay-project/assay/commits/main"
    );
    assert!(
        http.requests
            .iter()
            .all(|request| request.authorization().is_none())
    );
}

#[test]
fn named_refs_are_percent_encoded_and_only_returned_shas_are_accepted() {
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"assay","owner":{"login":"assay-project"},"default_branch":"main","private":false}"#,
        ),
        response(200, &rate_headers(), &format!(r#"{{"sha":"{REVISION}"}}"#)),
    ]);
    let repository = CanonicalGitHubRepository::parse("assay-project/assay").unwrap();
    GitHubCollector::new(&mut http)
        .resolve_revision(
            &repository,
            RevisionSelector::named("release/v1 candidate").unwrap(),
        )
        .unwrap();
    assert_eq!(
        http.requests[1].path(),
        "/repos/assay-project/assay/commits/release%2Fv1%20candidate"
    );

    let mut invalid = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"assay","owner":{"login":"assay-project"},"default_branch":"main","private":false}"#,
        ),
        response(200, &rate_headers(), r#"{"sha":"main"}"#),
    ]);
    let error = GitHubCollector::new(&mut invalid)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::InvalidProviderResponse);
}

#[test]
fn provider_response_extensions_are_ignored() {
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"assay","owner":{"login":"owner","id":9},"default_branch":"main","private":false,"visibility":"public"}"#,
        ),
        response(
            200,
            &rate_headers(),
            &format!(r#"{{"sha":"{REVISION}","url":"https://api.github.invalid/commit"}}"#),
        ),
    ]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    assert!(
        GitHubCollector::new(&mut http)
            .resolve_revision(&repository, RevisionSelector::DefaultBranch)
            .is_ok()
    );
}

#[test]
fn rate_limits_are_explicit_and_response_bodies_never_enter_errors() {
    let body = r#"{"message":"token secret-source /private/path"}"#;
    let mut http = FakeHttp::new(vec![response(
        403,
        &[
            ("x-ratelimit-limit", "60"),
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "2000000000"),
        ],
        body,
    )]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let error = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();

    assert_eq!(error.kind(), CollectionErrorKind::RateLimited);
    assert_eq!(
        error.rate_limit(),
        Some(&RateLimitState::Exhausted {
            limit: Some(60),
            reset_at_unix_seconds: Some(2_000_000_000),
            retry_after_seconds: None,
        })
    );
    assert!(!error.to_string().contains("secret-source"));
    assert!(!error.to_string().contains("private/path"));

    let mut secondary = FakeHttp::new(vec![response(429, &[("retry-after", "120")], body)]);
    let error = GitHubCollector::new(&mut secondary)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(
        error.rate_limit(),
        Some(&RateLimitState::SecondaryLimited {
            retry_after_seconds: Some(120)
        })
    );

    let mut secondary_403 = FakeHttp::new(vec![response(
        403,
        &[("retry-after", "30"), ("x-ratelimit-remaining", "12")],
        body,
    )]);
    let error = GitHubCollector::new(&mut secondary_403)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(
        error.rate_limit(),
        Some(&RateLimitState::SecondaryLimited {
            retry_after_seconds: Some(30)
        })
    );
}

#[test]
fn successful_collection_preserves_unknown_rate_limit_and_private_is_unavailable() {
    let mut unknown = FakeHttp::new(vec![
        response(
            200,
            &[],
            r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":false}"#,
        ),
        response(200, &[], &format!(r#"{{"sha":"{REVISION}"}}"#)),
    ]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    let resolved = GitHubCollector::new(&mut unknown)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap();
    assert_eq!(resolved.rate_limit(), &RateLimitState::Unknown);

    let mut private = FakeHttp::new(vec![response(
        200,
        &rate_headers(),
        r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":true}"#,
    )]);
    let error = GitHubCollector::new(&mut private)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::NotPublic);
}

struct FakeBlobCache;

impl BlobCacheLookup for FakeBlobCache {
    fn lookup(&self, key: &BlobAnalysisKey) -> BlobCacheState {
        match key.blob().as_str() {
            BLOB_A => BlobCacheState::Hit,
            BLOB_B => BlobCacheState::Miss,
            _ => BlobCacheState::Unavailable,
        }
    }
}

#[derive(Default)]
struct RecordingSink(Vec<BlobWorkItem>);

impl TreeSink for RecordingSink {
    fn accept(&mut self, item: BlobWorkItem) -> Result<(), TreeSinkError> {
        self.0.push(item);
        Ok(())
    }
}

fn tree_body(truncated: bool) -> String {
    format!(
        r#"{{"sha":"{REVISION}","truncated":{truncated},"tree":[
          {{"path":"package.json","mode":"100644","type":"blob","sha":"{BLOB_A}","size":120}},
          {{"path":"packages/api/package.json","mode":"100644","type":"blob","sha":"{BLOB_B}","size":90}},
          {{"path":"packages/api/src/lib.ts","mode":"100644","type":"blob","sha":"1111111111111111111111111111111111111111","size":1000}},
          {{"path":"vendor/tool","mode":"160000","type":"commit","sha":"2222222222222222222222222222222222222222"}},
          {{"path":"docs","mode":"040000","type":"tree","sha":"3333333333333333333333333333333333333333"}}
        ]}}"#
    )
}

#[test]
fn streams_tree_entries_skips_cached_blobs_and_detects_monorepo_boundaries() {
    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &tree_body(false))]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let revision = RevisionId::from_str(REVISION).unwrap();
    let mut sink = RecordingSink::default();
    let summary = GitHubCollector::new(&mut http)
        .stream_tree(
            &repository,
            &revision,
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::default(),
            &mut sink,
        )
        .unwrap();

    assert_eq!(summary.status(), CollectionStatus::Complete);
    assert_eq!(summary.observed_entries(), 5);
    assert_eq!(summary.observed_blobs(), 3);
    assert_eq!(summary.cache_hits(), 1);
    assert_eq!(summary.cache_misses(), 1);
    assert_eq!(summary.cache_unavailable(), 1);
    assert_eq!(summary.project_boundaries(), &[".", "packages/api"]);
    assert!(summary.partial_reasons().is_empty());
    assert_eq!(sink.0.len(), 2, "cache hits must not be reanalyzed");
    assert_eq!(sink.0[0].path(), "packages/api/package.json");
    assert_eq!(sink.0[0].cache_state(), BlobCacheState::Miss);
    assert_eq!(sink.0[1].cache_state(), BlobCacheState::Unavailable);
    assert_eq!(
        http.requests[0].path(),
        format!("/repos/owner/repository/git/trees/{REVISION}?recursive=1")
    );
}

#[test]
fn provider_truncation_and_local_bounds_produce_partial_not_zero_or_failure() {
    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &tree_body(true))]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let revision = RevisionId::from_str(REVISION).unwrap();
    let mut sink = RecordingSink::default();
    let summary = GitHubCollector::new(&mut http)
        .stream_tree(
            &repository,
            &revision,
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::new(2, 16 * 1024, 256, 1).unwrap(),
            &mut sink,
        )
        .unwrap();

    assert_eq!(summary.status(), CollectionStatus::Partial);
    assert_eq!(summary.observed_entries(), 5);
    assert_eq!(summary.observed_blobs(), 2);
    assert_eq!(summary.project_boundaries(), &["."]);
    assert!(
        summary
            .partial_reasons()
            .contains(&TreePartialReason::ProviderTruncated)
    );
    assert!(
        summary
            .partial_reasons()
            .contains(&TreePartialReason::EntryLimit)
    );
    assert!(
        summary
            .partial_reasons()
            .contains(&TreePartialReason::BoundaryLimit)
    );
}

#[test]
fn response_byte_limit_fails_closed_without_leaking_source_content() {
    let body = format!(
        r#"{{"sha":"{REVISION}","truncated":false,"tree":[{{"path":"private/secret-source.rs","mode":"100644","type":"blob","sha":"{BLOB_A}","size":120}}]}}"#
    );
    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &body)]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let mut sink = RecordingSink::default();
    let error = GitHubCollector::new(&mut http)
        .stream_tree(
            &repository,
            &RevisionId::from_str(REVISION).unwrap(),
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::new(10, 32, 256, 10).unwrap(),
            &mut sink,
        )
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::ResponseLimit);
    assert!(!error.to_string().contains("secret-source"));
    assert!(!error.to_string().contains("private/"));
}

#[test]
fn tree_sink_failures_are_stable_and_do_not_echo_paths() {
    struct FailingSink;
    impl TreeSink for FailingSink {
        fn accept(&mut self, _item: BlobWorkItem) -> Result<(), TreeSinkError> {
            Err(TreeSinkError::new("analysis_queue_unavailable").unwrap())
        }
    }

    struct MissCache;
    impl BlobCacheLookup for MissCache {
        fn lookup(&self, _key: &BlobAnalysisKey) -> BlobCacheState {
            BlobCacheState::Miss
        }
    }

    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &tree_body(false))]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let error = GitHubCollector::new(&mut http)
        .stream_tree(
            &repository,
            &RevisionId::from_str(REVISION).unwrap(),
            &MissCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::default(),
            &mut FailingSink,
        )
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::Sink);
    assert!(!error.to_string().contains("package.json"));
}

#[test]
fn limits_are_positive_and_overlong_paths_are_explicitly_partial() {
    assert!(TreeCollectionLimits::new(0, 1, 1, 1).is_err());
    let long_path = format!("packages/{}/package.json", "a".repeat(32));
    let body = format!(
        r#"{{"sha":"{REVISION}","truncated":false,"url":"ignored","tree":[{{"path":"{long_path}","mode":"100644","type":"blob","sha":"{BLOB_A}","size":120,"url":"ignored"}}]}}"#
    );
    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &body)]);
    let mut sink = RecordingSink::default();
    let summary = GitHubCollector::new(&mut http)
        .stream_tree(
            &CanonicalGitHubRepository::parse("owner/repository").unwrap(),
            &RevisionId::from_str(REVISION).unwrap(),
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::new(10, 16 * 1024, 16, 10).unwrap(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(summary.status(), CollectionStatus::Partial);
    assert_eq!(summary.observed_blobs(), 1);
    assert!(
        summary
            .partial_reasons()
            .contains(&TreePartialReason::PathLimit)
    );
    assert!(sink.0.is_empty());
}

#[test]
fn trailing_tree_payload_is_rejected_without_echoing_it() {
    let body = format!("{} source-content", tree_body(false));
    let mut http = FakeHttp::new(vec![response(200, &rate_headers(), &body)]);
    let mut sink = RecordingSink::default();
    let error = GitHubCollector::new(&mut http)
        .stream_tree(
            &CanonicalGitHubRepository::parse("owner/repository").unwrap(),
            &RevisionId::from_str(REVISION).unwrap(),
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::default(),
            &mut sink,
        )
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::InvalidProviderResponse);
    assert!(!error.to_string().contains("source-content"));
}
