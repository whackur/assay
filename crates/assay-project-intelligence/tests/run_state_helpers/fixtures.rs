use std::str::FromStr;

use assay_domain::ContentHash;
use assay_project_intelligence::{
    Administrator, ProjectRun, RetryPolicy, RunId, Stage, StageAttempt,
};

pub fn run_id() -> RunId {
    RunId::new("run-0000000042").unwrap()
}

pub fn snapshot(byte: char) -> ContentHash {
    ContentHash::from_str(&format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
}

pub fn completed() -> StageAttempt {
    StageAttempt::Completed(snapshot('a'))
}

pub fn new_run() -> ProjectRun {
    ProjectRun::new(run_id(), RetryPolicy::v1())
}

pub fn exhaust(run: &mut ProjectRun, stage: Stage, reason: &str) {
    for _ in 0..=RetryPolicy::v1().automatic_retry_budget() {
        let _ = run.record_attempt(
            stage,
            StageAttempt::Failed {
                reason: reason.to_owned(),
            },
        );
    }
    assert_eq!(
        run.stage_status(stage),
        assay_project_intelligence::StageStatus::Unavailable
    );
}

pub fn representative_run() -> ProjectRun {
    let admin = Administrator::assume();
    let mut run = new_run();
    for stage in [
        Stage::SourceVerification,
        Stage::RevisionPinning,
        Stage::FileAndHistoryAnalysis,
        Stage::ProjectTypeDetermination,
    ] {
        run.record_attempt(stage, completed()).unwrap();
    }
    run.record_attempt(
        Stage::CiAndDependencyEvidence,
        StageAttempt::PartiallyCompleted {
            snapshot: snapshot('b'),
            reason: "reported_ci_incomplete".to_owned(),
        },
    )
    .unwrap();
    exhaust(
        &mut run,
        Stage::SimilarProjectDiscovery,
        "cohort_provider_unavailable",
    );
    run.soft_delete(&admin, "2026-07-16T09:15:00Z").unwrap();
    run.restore(&admin, "2026-07-16T09:20:00Z").unwrap();
    run
}
