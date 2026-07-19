use std::str::FromStr;

use assay_domain::{RevisionId, RuleSetHash};
use assay_github::{
    BlobCacheLookup, BlobCacheState, CacheVersion, CanonicalGitHubRepository, CollectionErrorKind,
    CollectionStatus, GitHubCollector, TreeCollectionLimits, TreePartialReason, TreeSink,
    TreeSinkError,
};

use super::{
    BlobWorkItem, FakeBlobCache, FakeHttp, REVISION, RULES, RecordingSink, rate_headers, response,
    tree_body,
};

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
        r#"{{"sha":"{REVISION}","truncated":false,"tree":[{{"path":"private/secret-source.rs","mode":"100644","type":"blob","sha":"{}","size":120}}]}}"#,
        super::BLOB_A
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
        fn lookup(&self, _key: &assay_github::BlobAnalysisKey) -> BlobCacheState {
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
        r#"{{"sha":"{REVISION}","truncated":false,"url":"ignored","tree":[{{"path":"{long_path}","mode":"100644","type":"blob","sha":"{}","size":120,"url":"ignored"}}]}}"#,
        super::BLOB_A
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
fn a_lying_large_content_length_rejects_before_the_body_is_read() {
    let body = format!(
        r#"{{"sha":"{REVISION}","truncated":false,"tree":[{{"path":"private/secret-source.rs","mode":"100644","type":"blob","sha":"{}","size":120}}]}}"#,
        super::BLOB_A
    );
    let mut http = FakeHttp::new(vec![response(
        200,
        &[("content-length", "100000000")],
        &body,
    )]);
    let mut sink = RecordingSink::default();
    let error = GitHubCollector::new(&mut http)
        .stream_tree(
            &CanonicalGitHubRepository::parse("owner/repository").unwrap(),
            &RevisionId::from_str(REVISION).unwrap(),
            &FakeBlobCache,
            CacheVersion::parse("static-evidence-1").unwrap(),
            RuleSetHash::from_str(RULES).unwrap(),
            TreeCollectionLimits::new(10, 1_024, 256, 10).unwrap(),
            &mut sink,
        )
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::ResponseLimit);
    assert!(!error.to_string().contains("secret-source"));
    assert!(sink.0.is_empty());
}

#[test]
fn an_invalid_blob_object_id_fails_closed_rather_than_being_dropped() {
    let body = format!(
        r#"{{"sha":"{REVISION}","truncated":false,"tree":[{{"path":"src/private-secret.ts","mode":"100644","type":"blob","sha":"not-a-real-object-id","size":10}}]}}"#
    );
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
    assert!(!error.to_string().contains("private-secret"));
    assert!(sink.0.is_empty());
}

#[test]
fn path_traversal_entries_are_partial_and_never_reach_the_sink() {
    let body = format!(
        r#"{{"sha":"{REVISION}","truncated":false,"tree":[
          {{"path":"../escape.ts","mode":"100644","type":"blob","sha":"{}","size":10}},
          {{"path":"/absolute.ts","mode":"100644","type":"blob","sha":"{}","size":10}},
          {{"path":"nested/./relative.ts","mode":"100644","type":"blob","sha":"{}","size":10}}
        ]}}"#,
        super::BLOB_B,
        super::BLOB_B,
        super::BLOB_B
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
            TreeCollectionLimits::default(),
            &mut sink,
        )
        .unwrap();
    assert_eq!(summary.status(), CollectionStatus::Partial);
    assert_eq!(summary.observed_blobs(), 3);
    assert!(
        summary
            .partial_reasons()
            .contains(&TreePartialReason::PathLimit)
    );
    assert!(sink.0.is_empty(), "unsafe paths must not be streamed");
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
