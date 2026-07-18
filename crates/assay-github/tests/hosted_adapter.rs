use std::{collections::VecDeque, io::Cursor};

use assay_github::{
    GitHubHttp, GitHubRequest, GitHubResponse, HostedGitHubAdapter, RateLimitState, TransportError,
};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

struct FakeHttp {
    responses: VecDeque<GitHubResponse>,
    paths: Vec<String>,
}

impl GitHubHttp for FakeHttp {
    fn execute(&mut self, request: GitHubRequest) -> Result<GitHubResponse, TransportError> {
        self.paths.push(request.path().to_owned());
        Ok(self.responses.pop_front().expect("scripted response"))
    }
}

fn response(status: u16, headers: &[(&str, &str)], body: String) -> GitHubResponse {
    GitHubResponse::new(
        status,
        headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        Box::new(Cursor::new(body.into_bytes())),
    )
}

#[test]
fn hosted_projection_preserves_etag_identity_revision_and_normalized_facts() {
    let http = FakeHttp {
        responses: VecDeque::from([
            response(
                200,
                &[("etag", "metadata-v2")],
                r#"{"id":42,"name":"Assay","owner":{"login":"Whackur"},"default_branch":"main","private":false,"description":"Evidence tooling","stargazers_count":12,"forks_count":3,"open_issues_count":2,"archived":false,"fork":false,"license":{"spdx_id":"MIT"}}"#.to_owned(),
            ),
            response(
                200,
                &[
                    ("x-ratelimit-limit", "60"),
                    ("x-ratelimit-remaining", "59"),
                    ("x-ratelimit-reset", "2000000000"),
                ],
                format!(r#"{{"sha":"{SHA}"}}"#),
            ),
        ]),
        paths: Vec::new(),
    };
    let mut adapter = HostedGitHubAdapter::new(http);
    let collection = adapter.collect("whackur", "assay").unwrap();

    assert_eq!(collection.provider_repository_id, 42);
    assert_eq!(collection.owner, "whackur");
    assert_eq!(collection.etag.as_deref(), Some("metadata-v2"));
    assert_eq!(collection.head_sha, SHA);
    assert_eq!(collection.normalized_facts["stargazers_count"], 12);
    assert_eq!(collection.normalized_facts["license_spdx"], "MIT");
    assert_eq!(
        collection.source_url,
        "https://api.github.com/repos/whackur/assay"
    );
    let transport = adapter.into_transport();
    assert_eq!(
        transport.paths,
        ["/repos/whackur/assay", "/repos/whackur/assay/commits/main"]
    );
}

#[test]
fn redirect_and_rate_limit_are_classified_without_following_an_origin() {
    let redirect = FakeHttp {
        responses: VecDeque::from([response(
            302,
            &[("location", "https://evil.invalid/repository")],
            String::new(),
        )]),
        paths: Vec::new(),
    };
    let mut adapter = HostedGitHubAdapter::new(redirect);
    let failure = adapter.collect("whackur", "assay").unwrap_err();
    assert_eq!(failure.code(), "github_provider_failure");
    assert!(failure.retryable());
    assert!(failure.affects_provider_circuit());

    let limited = FakeHttp {
        responses: VecDeque::from([response(429, &[("retry-after", "30")], String::new())]),
        paths: Vec::new(),
    };
    let mut adapter = HostedGitHubAdapter::new(limited);
    let failure = adapter.collect("whackur", "assay").unwrap_err();
    assert_eq!(failure.code(), "github_rate_limited");
    assert!(failure.retryable());
    assert_eq!(failure.retry_after_seconds(), Some(30));

    let _rate_shape = RateLimitState::SecondaryLimited {
        retry_after_seconds: Some(30),
    };

    let missing = FakeHttp {
        responses: VecDeque::from([response(404, &[], String::new())]),
        paths: Vec::new(),
    };
    let mut adapter = HostedGitHubAdapter::new(missing);
    let failure = adapter.collect("whackur", "missing").unwrap_err();
    assert_eq!(failure.code(), "github_repository_unavailable");
    assert!(!failure.affects_provider_circuit());
}
