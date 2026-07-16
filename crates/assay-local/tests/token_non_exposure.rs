//! Proves a resolved GitHub PAT never reaches an argument, log, result, error,
//! stored record, or transport request. The token value is planted and then
//! searched for across every observable surface.

use std::fs;

use assay_local::{
    ConsentState, FetchOutcome, GithubTokenEnvVar, LocalHistoryStore, LocalReport, MapEnvironment,
    PrivateFetchRequest, PrivateGitTransport, SecretToken, TransportError, resolve_token,
};
use serde_json::json;
use tempfile::TempDir;

const PLANTED_TOKEN: &str = "ghp_LEAKME_0123456789_secret";

// A transport that records the request it was asked to perform. A correct
// implementation records the credential-free request and never the token.
struct RecordingTransport {
    recorded: std::cell::RefCell<Vec<String>>,
}

impl PrivateGitTransport for RecordingTransport {
    fn fetch(
        &self,
        request: &PrivateFetchRequest,
        authorization: Option<&SecretToken>,
    ) -> Result<FetchOutcome, TransportError> {
        // The header is built from the token but is not retained anywhere.
        let _header =
            authorization.map(|token| format!("Bearer {}", token.expose_for_authorization()));
        self.recorded
            .borrow_mut()
            .push(serde_json::to_string(request).unwrap());
        Ok(FetchOutcome::new(request.revision()))
    }
}

fn assert_absent(surface: &str, haystack: &str) {
    assert!(
        !haystack.contains(PLANTED_TOKEN),
        "token leaked into {surface}: {haystack}"
    );
}

#[test]
fn token_never_appears_on_any_observable_surface() {
    let environment = MapEnvironment::default().with("GITHUB_TOKEN", PLANTED_TOKEN);
    let var = GithubTokenEnvVar::parse("GITHUB_TOKEN").unwrap();
    let token = resolve_token(&environment, &var).unwrap();

    // Positive control: the exposed value really is the planted token, so the
    // absence assertions below have teeth.
    assert_eq!(token.expose_for_authorization(), PLANTED_TOKEN);

    // Debug of the secret is redacted.
    assert_absent("secret debug", &format!("{token:?}"));

    // The transport receives the token but records only the request.
    let transport = RecordingTransport {
        recorded: std::cell::RefCell::new(Vec::new()),
    };
    let request = PrivateFetchRequest::new("org/private-repository", "HEAD");
    transport.fetch(&request, Some(&token)).unwrap();
    assert_absent(
        "transport request",
        &serde_json::to_string(&request).unwrap(),
    );
    for recorded in transport.recorded.borrow().iter() {
        assert_absent("transport log", recorded);
    }

    // A missing-variable error names the variable, never the value.
    let empty = MapEnvironment::default();
    let error = resolve_token(&empty, &var).unwrap_err();
    assert_absent("resolution error", &error.to_string());

    // The persisted local report and its on-disk record hold no token.
    let analysis = json!({
        "schema_version": "1.0.0",
        "manifest": { "source_snapshot": { "source": {
            "kind": "local", "repository_id": "abc123"
        } } },
        "evidence": []
    });
    let report =
        LocalReport::from_analysis(analysis, &ConsentState::default(), "2026-07-16T00:00:00Z")
            .unwrap();
    assert_absent("report json", &report.to_value().to_string());

    let dir = TempDir::new().unwrap();
    let store = LocalHistoryStore::open(dir.path()).unwrap();
    let record = store
        .append(report.to_value(), "2026-07-16T00:00:00Z")
        .unwrap();
    assert_absent("stored record", &record.report().to_string());

    for entry in fs::read_dir(dir.path().join("records")).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        assert_absent("record file bytes", &String::from_utf8_lossy(&bytes));
    }
}
