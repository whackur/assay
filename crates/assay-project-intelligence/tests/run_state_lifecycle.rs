use assay_project_intelligence::{
    AdminAction, Administrator, AttemptDisposition, PIPELINE_STAGES, RetryPolicy, RunErrorKind,
    RunLifecycle, Stage, StageAttempt, StageStatus,
};

mod run_state_helpers;
use run_state_helpers::{completed, exhaust, new_run, snapshot};

#[test]
fn new_run_starts_every_named_stage_pending_without_a_user_retry_path() {
    let run = new_run();
    assert_eq!(PIPELINE_STAGES.len(), 9);
    for stage in PIPELINE_STAGES {
        assert_eq!(run.stage_status(stage), StageStatus::Pending);
        assert_eq!(run.stage_attempts(stage), 0);
        assert!(run.stage_reason(stage).is_none());
    }
    assert_eq!(run.status(), StageStatus::Pending);
    assert_eq!(run.lifecycle(), RunLifecycle::Active);
    assert!(!run.ordinary_user_retry_available());
}

#[test]
fn a_partial_stage_failure_preserves_completed_stages_only_marking_the_failure() {
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();
    run.record_attempt(Stage::RevisionPinning, completed())
        .unwrap();
    let disposition = run
        .record_attempt(
            Stage::CiAndDependencyEvidence,
            StageAttempt::PartiallyCompleted {
                snapshot: snapshot('b'),
                reason: "reported_ci_incomplete".to_owned(),
            },
        )
        .unwrap();

    assert_eq!(disposition, AttemptDisposition::Settled);
    assert_eq!(
        run.stage_status(Stage::SourceVerification),
        StageStatus::Complete
    );
    assert_eq!(
        run.stage_status(Stage::CiAndDependencyEvidence),
        StageStatus::Partial
    );
    assert_eq!(
        run.stage_reason(Stage::CiAndDependencyEvidence),
        Some("reported_ci_incomplete")
    );
    // Downstream stages that never ran stay pending, not zeroed or failed.
    assert_eq!(
        run.stage_status(Stage::ResultPublication),
        StageStatus::Pending
    );
    // A mixed run is partial: unavailable and partial are never a false success.
    assert_eq!(run.status(), StageStatus::Partial);
}

#[test]
fn automatic_retries_are_bounded_by_policy_then_terminate_without_user_retry() {
    let mut run = new_run();
    let budget = RetryPolicy::v1().automatic_retry_budget();
    assert_eq!(budget, 2);

    for _ in 0..budget {
        let disposition = run
            .record_attempt(
                Stage::SimilarProjectDiscovery,
                StageAttempt::Failed {
                    reason: "cohort_provider_unavailable".to_owned(),
                },
            )
            .unwrap();
        assert_eq!(disposition, AttemptDisposition::RetryScheduled);
        assert_eq!(
            run.stage_status(Stage::SimilarProjectDiscovery),
            StageStatus::Pending
        );
    }

    let disposition = run
        .record_attempt(
            Stage::SimilarProjectDiscovery,
            StageAttempt::Failed {
                reason: "cohort_provider_unavailable".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(disposition, AttemptDisposition::Exhausted);
    assert_eq!(
        run.stage_status(Stage::SimilarProjectDiscovery),
        StageStatus::Unavailable
    );
    assert!(run.stage_retries_exhausted(Stage::SimilarProjectDiscovery));
    assert_eq!(
        run.stage_attempts(Stage::SimilarProjectDiscovery),
        budget + 1
    );

    // Exhausted is terminal: there is no automatic or ordinary-user retry path.
    let closed = run.record_attempt(
        Stage::SimilarProjectDiscovery,
        StageAttempt::Failed {
            reason: "cohort_provider_unavailable".to_owned(),
        },
    );
    assert_eq!(closed.unwrap_err().kind(), RunErrorKind::StageNotPending);
    assert!(!run.ordinary_user_retry_available());
}

#[test]
fn recording_an_attempt_on_a_completed_stage_is_rejected() {
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();
    let error = run
        .record_attempt(Stage::SourceVerification, completed())
        .unwrap_err();
    assert_eq!(error.kind(), RunErrorKind::StageNotPending);
}

#[test]
fn administrator_reruns_only_a_failed_stage_reusing_completed_snapshots() {
    let admin = Administrator::assume();
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();
    exhaust(&mut run, Stage::SimilarProjectDiscovery, "provider_down");

    // A completed stage cannot be rerun; only failures are re-runnable.
    let rejected = run
        .rerun_stage(Stage::SourceVerification, &admin, "2026-07-16T09:00:00Z")
        .unwrap_err();
    assert_eq!(rejected.kind(), RunErrorKind::StageNotFailed);

    let event = run
        .rerun_stage(
            Stage::SimilarProjectDiscovery,
            &admin,
            "2026-07-16T09:15:00Z",
        )
        .unwrap();
    assert_eq!(event.action(), AdminAction::RerunStage);
    assert_eq!(event.stage(), Some(Stage::SimilarProjectDiscovery));
    // The failed stage resets; the completed stage keeps its immutable snapshot.
    assert_eq!(
        run.stage_status(Stage::SimilarProjectDiscovery),
        StageStatus::Pending
    );
    assert_eq!(run.stage_attempts(Stage::SimilarProjectDiscovery), 0);
    assert_eq!(
        run.stage_status(Stage::SourceVerification),
        StageStatus::Complete
    );
    assert_eq!(run.audit_events().len(), 1);
}

#[test]
fn rerun_failed_stages_resets_all_failures_and_refuses_when_none_failed() {
    let admin = Administrator::assume();
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();

    let nothing = run
        .rerun_failed_stages(&admin, "2026-07-16T10:00:00Z")
        .unwrap_err();
    assert_eq!(nothing.kind(), RunErrorKind::NothingToRerun);

    exhaust(&mut run, Stage::CiAndDependencyEvidence, "ci_unreachable");
    exhaust(&mut run, Stage::SimilarProjectDiscovery, "provider_down");
    run.rerun_failed_stages(&admin, "2026-07-16T10:05:00Z")
        .unwrap();

    assert_eq!(
        run.stage_status(Stage::CiAndDependencyEvidence),
        StageStatus::Pending
    );
    assert_eq!(
        run.stage_status(Stage::SimilarProjectDiscovery),
        StageStatus::Pending
    );
    assert_eq!(
        run.stage_status(Stage::SourceVerification),
        StageStatus::Complete
    );
}

#[test]
fn soft_delete_restore_and_purge_are_audited_lifecycle_transitions() {
    let admin = Administrator::assume();
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();

    run.soft_delete(&admin, "2026-07-16T11:00:00Z").unwrap();
    assert_eq!(run.lifecycle(), RunLifecycle::Deleted);
    // A deleted run is not active: no ordinary recording proceeds.
    assert_eq!(
        run.record_attempt(Stage::RevisionPinning, completed())
            .unwrap_err()
            .kind(),
        RunErrorKind::RunNotActive
    );

    run.restore(&admin, "2026-07-16T11:05:00Z").unwrap();
    assert_eq!(run.lifecycle(), RunLifecycle::Active);

    run.purge(&admin, "2026-07-16T11:10:00Z").unwrap();
    assert_eq!(run.lifecycle(), RunLifecycle::Purged);

    let actions: Vec<AdminAction> = run.audit_events().iter().map(|e| e.action()).collect();
    assert_eq!(
        actions,
        vec![
            AdminAction::SoftDelete,
            AdminAction::Restore,
            AdminAction::Purge
        ]
    );
}

#[test]
fn purge_drops_result_content_but_retains_the_audit_trail() {
    let admin = Administrator::assume();
    let mut run = new_run();
    run.record_attempt(Stage::SourceVerification, completed())
        .unwrap();
    run.purge(&admin, "2026-07-16T12:00:00Z").unwrap();

    let value = run.to_machine_value();
    let stage = value["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["stage"] == "source_verification")
        .unwrap();
    assert_eq!(stage["result_snapshot"], serde_json::Value::Null);
    assert_eq!(run.audit_events().len(), 1);
}

#[test]
fn invalid_lifecycle_transitions_and_inputs_are_rejected_without_echoing_values() {
    let admin = Administrator::assume();
    let mut run = new_run();

    assert_eq!(
        run.restore(&admin, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .kind(),
        RunErrorKind::InvalidLifecycleTransition
    );
    assert_eq!(
        run.record_attempt(
            Stage::SourceVerification,
            StageAttempt::Failed {
                reason: "Not A Code".to_owned()
            }
        )
        .unwrap_err()
        .kind(),
        RunErrorKind::InvalidReason
    );
    assert_eq!(
        run.soft_delete(&admin, "has\ttab").unwrap_err().kind(),
        RunErrorKind::InvalidTimestamp
    );
    assert!(assay_project_intelligence::RunId::new("../escape").is_err());
    assert!(assay_project_intelligence::RunId::new("Run-1").is_err());
}
