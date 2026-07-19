use assay_github::{
    CanonicalGitHubRepository, CollectionErrorKind, GitHubCollector, GitHubRequest, RateLimitState,
    RevisionSelector,
};

use super::{FakeHttp, REVISION, rate_headers, response};

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

#[test]
fn redirect_status_is_not_followed_and_never_leaks_location() {
    for status in [301_u16, 302, 307, 308] {
        let mut http = FakeHttp::new(vec![response(
            status,
            &[("location", "https://attacker.example/private-source")],
            r#"{"message":"moved"}"#,
        )]);
        let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
        let error = GitHubCollector::new(&mut http)
            .resolve_revision(&repository, RevisionSelector::DefaultBranch)
            .unwrap_err();
        assert_eq!(error.kind(), CollectionErrorKind::HttpStatus);
        assert!(!error.to_string().contains("attacker.example"));
        assert!(!error.to_string().contains("private-source"));
        assert_eq!(http.requests.len(), 1, "a redirect must not be followed");
    }
}

#[test]
fn duplicate_privacy_field_in_metadata_cannot_reopen_a_private_repository() {
    let mut http = FakeHttp::new(vec![response(
        200,
        &rate_headers(),
        r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":true,"private":false}"#,
    )]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    let result = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch);
    match result {
        Err(error) => assert!(matches!(
            error.kind(),
            CollectionErrorKind::NotPublic | CollectionErrorKind::InvalidProviderResponse
        )),
        Ok(_) => panic!("a duplicated privacy field must never resolve as public"),
    }
}

#[test]
fn oversized_revision_response_fails_closed_without_leaking_its_body() {
    let padding = "leak".repeat(150_000);
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":false}"#,
        ),
        response(
            200,
            &rate_headers(),
            &format!(r#"{{"sha":"{REVISION}","note":"{padding}"}}"#),
        ),
    ]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    let error = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(error.kind(), CollectionErrorKind::ResponseLimit);
    assert!(!error.to_string().contains("leak"));
}

#[test]
fn malformed_rate_headers_are_unknown_and_never_read_as_unlimited() {
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &[
                ("x-ratelimit-limit", "sixty"),
                ("x-ratelimit-remaining", "-1"),
                ("x-ratelimit-reset", ""),
            ],
            r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":false}"#,
        ),
        response(
            200,
            &[("x-ratelimit-remaining", "0x10")],
            &format!(r#"{{"sha":"{REVISION}"}}"#),
        ),
    ]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    let resolved = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap();
    assert_eq!(resolved.rate_limit(), &RateLimitState::Unknown);
}
