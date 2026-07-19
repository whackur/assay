use assay_github::{CanonicalGitHubRepository, GitHubCollector, RateLimitState, RevisionSelector};

use super::{FakeHttp, REVISION, rate_headers, response};

#[test]
fn http_date_retry_after_does_not_fabricate_a_numeric_delay() {
    let mut http = FakeHttp::new(vec![response(
        429,
        &[("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")],
        r#"{"message":"secondary limit"}"#,
    )]);
    let repository = CanonicalGitHubRepository::parse("owner/repository").unwrap();
    let error = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap_err();
    assert_eq!(
        error.rate_limit(),
        Some(&RateLimitState::SecondaryLimited {
            retry_after_seconds: None,
        })
    );
}

#[test]
fn a_successful_resolution_still_reports_an_exhausted_budget() {
    let mut http = FakeHttp::new(vec![
        response(
            200,
            &rate_headers(),
            r#"{"id":42,"name":"assay","owner":{"login":"owner"},"default_branch":"main","private":false}"#,
        ),
        response(
            200,
            &[
                ("x-ratelimit-limit", "60"),
                ("x-ratelimit-remaining", "0"),
                ("x-ratelimit-reset", "2000000000"),
            ],
            &format!(r#"{{"sha":"{REVISION}"}}"#),
        ),
    ]);
    let repository = CanonicalGitHubRepository::parse("owner/assay").unwrap();
    let resolved = GitHubCollector::new(&mut http)
        .resolve_revision(&repository, RevisionSelector::DefaultBranch)
        .unwrap();
    assert_eq!(
        resolved.rate_limit(),
        &RateLimitState::Exhausted {
            limit: Some(60),
            reset_at_unix_seconds: Some(2_000_000_000),
            retry_after_seconds: None,
        }
    );
}
