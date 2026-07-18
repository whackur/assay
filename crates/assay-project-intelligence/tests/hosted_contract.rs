use assay_project_intelligence::{
    HostedAdmission, HostedContractEnvelope, HostedErrorResponse, HostedEvaluationStatus,
    HostedJobStage, HostedJobState, HostedProjectStatus, HostedProjectStatusRecord,
    HostedRecentSourceStatus, HostedRecentSourceStatusRecord, HostedRequestState,
    HostedScoreStatus, HostedSubmission,
};
use jsonschema::Draft;
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

fn schema_validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/hosted-api/1.0.0.json")).unwrap();
    jsonschema::draft202012::meta::validate(&schema).unwrap();
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .unwrap()
}

fn assert_valid(value: Value) {
    if let Err(error) = schema_validator().validate(&value) {
        panic!("hosted API response failed schema validation: {error}: {value}");
    }
}

#[test]
fn project_intelligence_owns_schema_valid_hosted_projections() {
    let now = OffsetDateTime::from_unix_timestamp(1_768_651_200).unwrap();
    let submission = HostedSubmission::new(
        Uuid::nil(),
        "whackur".to_owned(),
        "assay".to_owned(),
        HostedRequestState::Queued,
        HostedAdmission::Admitted,
        None,
    );
    assert_valid(serde_json::to_value(HostedContractEnvelope::new(submission)).unwrap());

    let project = HostedProjectStatus::project(HostedProjectStatusRecord {
        request_id: Uuid::nil(),
        owner: "whackur".to_owned(),
        repository: "assay".to_owned(),
        canonical_url: "https://github.com/whackur/assay".to_owned(),
        request_state: HostedRequestState::Collecting,
        job_stage: HostedJobStage::Evaluating,
        job_state: HostedJobState::Running,
        last_error_code: None,
        provider_repository_id: Some(42),
        default_branch: Some("main".to_owned()),
        head_sha: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        description: Some("Evidence tooling".to_owned()),
        stars: Some(12),
        evaluation_status: Some(HostedEvaluationStatus::ValidatedUnpublished),
        score_status: HostedScoreStatus::Pending,
        next_attempt_at: now,
        updated_at: now,
    });
    assert_valid(serde_json::to_value(HostedContractEnvelope::new(project)).unwrap());

    let recent = HostedRecentSourceStatus::recent_source(HostedRecentSourceStatusRecord {
        owner: "whackur".to_owned(),
        repository: "assay".to_owned(),
        canonical_url: "https://github.com/whackur/assay".to_owned(),
        provider_repository_id: 42,
        description: None,
        stars: Some(12),
        default_branch: Some("main".to_owned()),
        head_sha: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        collection_status: HostedRequestState::Complete,
        evaluation_status: Some(HostedEvaluationStatus::Unavailable),
        score_status: HostedScoreStatus::Unavailable,
        updated_at: now,
    });
    assert_valid(serde_json::to_value(HostedContractEnvelope::new(vec![recent])).unwrap());
    assert_valid(serde_json::to_value(HostedErrorResponse::new("project_not_found")).unwrap());
}

#[test]
fn persistence_values_are_typed_and_running_normalizes_to_collecting() {
    assert_eq!(
        HostedRequestState::try_from("running").unwrap(),
        HostedRequestState::Collecting
    );
    assert!(HostedEvaluationStatus::try_from("published").is_err());
}

#[test]
fn schema_rejects_numeric_unavailable_score() {
    let invalid = json!({
        "contract": "assay-hosted-api",
        "schema_version": "1.0.0",
        "data": {
            "owner": "whackur",
            "repository": "assay",
            "canonical_url": "https://github.com/whackur/assay",
            "provider_repository_id": "42",
            "description": null,
            "stars": 12,
            "default_branch": "main",
            "head_sha": "0123456789abcdef0123456789abcdef01234567",
            "collection_status": "complete",
            "evaluation_status": "unavailable",
            "score_status": 0,
            "updated_at": "2026-07-18T12:00:00Z"
        }
    });
    assert!(schema_validator().validate(&invalid).is_err());
}
