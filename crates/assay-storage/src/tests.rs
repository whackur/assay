use assay_project_intelligence::{HostedAdmission, HostedRequestState};
use uuid::Uuid;

use crate::admission::submission;
use crate::failure::retry_backoff_seconds;
use crate::failure_circuit::failure_circuit_buckets;
use crate::rows::{ReconcileDisposition, reconcile_disposition};
use crate::util::{canonical_json, sha256_hex};

#[test]
fn snapshot_hash_changes_when_normalized_facts_change() {
    let first = sha256_hex(canonical_json(&serde_json::json!({"stars": 1})).as_bytes());
    let second = sha256_hex(canonical_json(&serde_json::json!({"stars": 2})).as_bytes());
    assert_ne!(first, second);
}

#[test]
fn public_submission_reports_bounded_cooldown_without_fabricating_a_result() {
    let value = submission(
        Uuid::nil(),
        "owner",
        "repo",
        "complete",
        "cooldown",
        Some(60),
    );
    assert_eq!(value.retry_after_seconds(), Some(60));
    assert_eq!(value.admission(), HostedAdmission::Cooldown);
}

#[test]
fn active_duplicate_uses_the_schema_defined_admission_and_state() {
    let value = submission(
        Uuid::nil(),
        "owner",
        "repo",
        "running",
        "joined_active",
        None,
    );
    assert_eq!(value.state(), HostedRequestState::Collecting);
    assert_eq!(value.admission(), HostedAdmission::JoinedActive);
}

#[test]
fn retry_policy_backs_off_and_stops_at_the_attempt_budget() {
    assert_eq!(retry_backoff_seconds(1, 3, true, None, 3_600), Some(4));
    assert_eq!(retry_backoff_seconds(2, 3, true, None, 3_600), Some(8));
    assert_eq!(retry_backoff_seconds(3, 3, true, None, 3_600), None);
    assert_eq!(retry_backoff_seconds(1, 3, false, None, 3_600), None);
}

#[test]
fn provider_retry_after_wins_without_consuming_attempts_early() {
    assert_eq!(
        retry_backoff_seconds(1, 3, true, Some(900), 3_600),
        Some(900)
    );
    assert_eq!(
        retry_backoff_seconds(1, 3, true, Some(7_200), 3_600),
        Some(3_600)
    );
}

#[test]
fn repository_failures_do_not_increment_the_provider_circuit() {
    let buckets = failure_circuit_buckets(Some("anonymous"), Some("owner"), false);
    assert_eq!(
        buckets,
        [
            ("anonymous_client", "anonymous"),
            ("repository_owner", "owner")
        ]
    );
}

#[test]
fn provider_failures_increment_all_applicable_circuits() {
    let buckets = failure_circuit_buckets(Some("anonymous"), Some("owner"), true);
    assert_eq!(
        buckets,
        [
            ("anonymous_client", "anonymous"),
            ("repository_owner", "owner"),
            ("provider", "github-ollama")
        ]
    );
}

#[test]
fn exhausted_abandoned_jobs_terminally_release_capacity() {
    assert_eq!(
        reconcile_disposition(true, 3, 3),
        ReconcileDisposition::TerminalRelease
    );
    assert_eq!(
        reconcile_disposition(true, 2, 3),
        ReconcileDisposition::Retry
    );
    assert_eq!(
        reconcile_disposition(false, 1, 3),
        ReconcileDisposition::TerminalRelease
    );
}
