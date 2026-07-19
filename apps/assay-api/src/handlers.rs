use assay_github::CanonicalGitHubRepository;
use assay_project_intelligence::{
    HostedAdmission, HostedContractEnvelope, HostedProjectStatus, HostedRecentSourceStatus,
    HostedSubmission, HostedSubmissionRequest,
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
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
}
