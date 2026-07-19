use assay_github::CanonicalGitHubRepository;
use assay_project_intelligence::{
    HostedAdmission, HostedContractEnvelope, HostedProjectStatus, HostedRecentSourceStatus,
    HostedSubmission, HostedSubmissionRequest,
};
use assay_storage::PublicationApproval;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::{ANONYMOUS_BUCKET_HEADER, AppState, SHARED_ANONYMOUS_BUCKET};

pub(crate) async fn live() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

pub(crate) async fn ready(State(state): State<AppState>) -> Response {
    match state.storage.health().await {
        Ok(()) => (axum::http::StatusCode::OK, Json(json!({"status": "ready"}))).into_response(),
        Err(_) => ApiError::service_unavailable("database_unavailable").into_response(),
    }
}

pub(crate) async fn recent_sources(
    State(state): State<AppState>,
) -> Result<Json<HostedContractEnvelope<Vec<HostedRecentSourceStatus>>>, ApiError> {
    let projects = state
        .storage
        .hosted_recent_source_statuses(50)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(HostedContractEnvelope::new(projects)))
}

pub(crate) async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HostedSubmissionRequest>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<HostedContractEnvelope<HostedSubmission>>,
    ),
    ApiError,
> {
    let repository = CanonicalGitHubRepository::parse(&input.repository)
        .map_err(|_| ApiError::bad_request("invalid_github_repository"))?;
    let submission = state
        .storage
        .submit_public(
            repository.owner(),
            repository.name(),
            anonymous_bucket(&headers),
            state.admission_limits,
        )
        .await
        .map_err(ApiError::admission)?;
    let status = if submission.admission() == HostedAdmission::Admitted {
        axum::http::StatusCode::ACCEPTED
    } else {
        axum::http::StatusCode::OK
    };
    Ok((status, Json(HostedContractEnvelope::new(submission))))
}

pub(crate) async fn status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<HostedContractEnvelope<HostedProjectStatus>>, ApiError> {
    let project = state
        .storage
        .hosted_status_by_request(id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("submission_not_found"))?;
    Ok(Json(HostedContractEnvelope::new(project)))
}

pub(crate) async fn project(
    State(state): State<AppState>,
    Path((owner, repository)): Path<(String, String)>,
) -> Result<Json<HostedContractEnvelope<HostedProjectStatus>>, ApiError> {
    let canonical = CanonicalGitHubRepository::parse(&format!("{owner}/{repository}"))
        .map_err(|_| ApiError::bad_request("invalid_github_repository"))?;
    let project = state
        .storage
        .hosted_status_by_repository(canonical.owner(), canonical.name())
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("project_not_found"))?;
    Ok(Json(HostedContractEnvelope::new(project)))
}

pub(crate) async fn ai_analysis(
    State(state): State<AppState>,
    Path((owner, repository)): Path<(String, String)>,
) -> Result<Json<assay_project_intelligence::ProjectAiAnalysisEnvelope>, ApiError> {
    let canonical = CanonicalGitHubRepository::parse(&format!("{owner}/{repository}"))
        .map_err(|_| ApiError::bad_request("invalid_github_repository"))?;
    let analysis = state
        .storage
        .project_ai_analysis_by_repository(canonical.owner(), canonical.name())
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("project_ai_analysis_not_found"))?;
    Ok(Json(analysis))
}

pub(crate) async fn approve_ai_analysis(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> Result<StatusCode, ApiError> {
    require_internal_admin(&headers, &state)?;
    let evaluation_snapshot_id = input
        .get("evaluation_snapshot_id")
        .and_then(|v| v.as_str())
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| ApiError::bad_request("invalid_evaluation_snapshot_id"))?;
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty() && v.len() <= 256)
            .map(str::to_owned)
            .ok_or_else(|| ApiError::bad_request("approval_identity_required"))
    };
    state
        .storage
        .approve_public_ai_analysis(&PublicationApproval {
            evaluation_snapshot_id,
            issuer: header("x-assay-identity-issuer")?,
            subject: header("x-assay-identity-subject")?,
            display_name: header("x-assay-identity-display-name")?,
        })
        .await
        .map_err(|error| match error {
            assay_storage::StorageError::PublicationNotFound => {
                ApiError::not_found("evaluation_snapshot_not_found")
            }
            assay_storage::StorageError::PublicationNotSafe => {
                ApiError::bad_request("evaluation_not_safe_to_publish")
            }
            assay_storage::StorageError::Database(error) => ApiError::database(error),
            _ => ApiError::service_unavailable("publication_unavailable"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn review_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<
    Json<assay_project_intelligence::HostedContractEnvelope<Vec<assay_storage::ReviewQueueItem>>>,
    ApiError,
> {
    require_internal_admin(&headers, &state)?;
    let items = state
        .storage
        .hosted_ai_analysis_review_queue()
        .await
        .map_err(ApiError::database)?;
    Ok(Json(
        assay_project_intelligence::HostedContractEnvelope::new(items),
    ))
}

fn require_internal_admin(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let Some(value) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return Err(ApiError::unauthorized("internal_authorization_required"));
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(ApiError::unauthorized("internal_authorization_required"));
    };
    if !constant_time_equal(token.as_bytes(), state.internal_admin_token.as_bytes()) {
        return Err(ApiError::unauthorized("internal_authorization_required"));
    }
    Ok(())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = (left.len() ^ right.len()) as u8;
    for index in 0..left.len().max(right.len()) {
        difference |=
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0);
    }
    difference == 0
}

pub(crate) fn anonymous_bucket(headers: &HeaderMap) -> &str {
    headers
        .get(ANONYMOUS_BUCKET_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .unwrap_or(SHARED_ANONYMOUS_BUCKET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_bounded_lowercase_one_way_bucket_is_accepted() {
        let mut headers = HeaderMap::new();
        headers.insert(ANONYMOUS_BUCKET_HEADER, "A".repeat(64).parse().unwrap());
        assert_eq!(anonymous_bucket(&headers), SHARED_ANONYMOUS_BUCKET);
        let expected = "a".repeat(64);
        headers.insert(ANONYMOUS_BUCKET_HEADER, expected.parse().unwrap());
        assert_eq!(anonymous_bucket(&headers), "a".repeat(64));
    }

    #[test]
    fn bearer_comparison_is_exact() {
        assert!(constant_time_equal(b"secret", b"secret"));
        assert!(!constant_time_equal(b"secret", b"secreT"));
        assert!(!constant_time_equal(b"secret", b"secret-longer"));
    }
}
