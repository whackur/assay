use std::{env, net::SocketAddr, process::ExitCode};

use assay_github::CanonicalGitHubRepository;
use assay_project_intelligence::{
    HostedAdmission, HostedContractEnvelope, HostedErrorResponse, HostedProjectStatus,
    HostedRecentSourceStatus, HostedSubmission, HostedSubmissionRequest,
};
use assay_storage::{AdmissionError, PublicAdmissionLimits, Storage};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use uuid::Uuid;

const ANONYMOUS_BUCKET_HEADER: &str = "x-assay-anonymous-bucket-id";
const SHARED_ANONYMOUS_BUCKET: &str =
    "488711212647543ea7c62e9193c7492ee9c97d89a24b6ae8f98ccb4efe228c96";

#[derive(Clone)]
struct AppState {
    storage: Storage,
    admission_limits: PublicAdmissionLimits,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("assay-api startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = required_env("DATABASE_URL")?;
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;
    let admission_limits = PublicAdmissionLimits {
        max_active: bounded_env("ASSAY_MAX_ACTIVE_JOBS", 8, 1, 1_000)?,
        completed_cooldown_seconds: bounded_env(
            "ASSAY_PUBLIC_COMPLETED_COOLDOWN_SECONDS",
            14 * 24 * 60 * 60,
            60,
            30 * 24 * 60 * 60,
        )?,
        failure_backoff_seconds: bounded_env(
            "ASSAY_PUBLIC_FAILURE_BACKOFF_SECONDS",
            60,
            10,
            60 * 60,
        )?,
        bucket_window_seconds: bounded_env("ASSAY_PUBLIC_BUCKET_WINDOW_SECONDS", 600, 10, 86_400)?,
        bucket_cooldown_seconds: bounded_env(
            "ASSAY_PUBLIC_BUCKET_COOLDOWN_SECONDS",
            900,
            10,
            86_400,
        )?,
        anonymous_burst: bounded_env("ASSAY_PUBLIC_ANONYMOUS_BURST", 4, 1, 1_000)? as i32,
        owner_burst: bounded_env("ASSAY_PUBLIC_OWNER_BURST", 3, 1, 1_000)? as i32,
        provider_burst: bounded_env("ASSAY_PUBLIC_PROVIDER_BURST", 20, 1, 10_000)? as i32,
    };
    seed_repositories(&storage, admission_limits.max_active).await;
    let state = AppState {
        storage,
        admission_limits,
    };
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/internal/hosted/recent-sources", get(recent_sources))
        .route("/internal/hosted/submissions", post(submit))
        .route("/internal/hosted/submissions/{id}", get(status))
        .route(
            "/internal/hosted/projects/github/{owner}/{repository}",
            get(project),
        )
        .layer(DefaultBodyLimit::max(4 * 1024))
        .with_state(state);
    let address: SocketAddr = env::var("ASSAY_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("assay-api listening on {address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn live() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

async fn ready(State(state): State<AppState>) -> Response {
    match state.storage.health().await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "ready"}))).into_response(),
        Err(_) => ApiError::service_unavailable("database_unavailable").into_response(),
    }
}

async fn recent_sources(
    State(state): State<AppState>,
) -> Result<Json<HostedContractEnvelope<Vec<HostedRecentSourceStatus>>>, ApiError> {
    let projects = state
        .storage
        .hosted_recent_source_statuses(50)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(HostedContractEnvelope::new(projects)))
}

async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<HostedSubmissionRequest>,
) -> Result<(StatusCode, Json<HostedContractEnvelope<HostedSubmission>>), ApiError> {
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
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(HostedContractEnvelope::new(submission))))
}

fn anonymous_bucket(headers: &HeaderMap) -> &str {
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

async fn status(
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

async fn project(
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

async fn seed_repositories(storage: &Storage, max_active_jobs: i64) {
    let seeds = env::var("ASSAY_SEED_REPOSITORIES").unwrap_or_else(|_| "whackur/assay".to_owned());
    for raw in seeds
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        match CanonicalGitHubRepository::parse(raw) {
            Ok(repository) => {
                if let Err(error) = storage
                    .submit_seed(repository.owner(), repository.name(), max_active_jobs)
                    .await
                {
                    eprintln!(
                        "seed admission failed for {}: {error}",
                        repository.identifier()
                    );
                }
            }
            Err(_) => eprintln!("ignored invalid ASSAY_SEED_REPOSITORIES entry"),
        }
    }
}

fn required_env(name: &'static str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("{name} is required").into())
}

fn bounded_env(
    name: &'static str,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let value = match env::var(name) {
        Ok(value) => value.parse::<i64>()?,
        Err(_) => default,
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}").into());
    }
    Ok(value)
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    retry_after_seconds: Option<i64>,
}

impl ApiError {
    fn bad_request(code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            retry_after_seconds: None,
        }
    }

    fn not_found(code: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            retry_after_seconds: None,
        }
    }

    fn service_unavailable(code: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code,
            retry_after_seconds: None,
        }
    }

    fn database(_: sqlx::Error) -> Self {
        Self::service_unavailable("database_unavailable")
    }

    fn admission(error: AdmissionError) -> Self {
        match error {
            AdmissionError::CapacityFull => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "analysis_capacity_full",
                retry_after_seconds: None,
            },
            AdmissionError::RateLimited {
                scope,
                retry_after_seconds,
            } => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: match scope {
                    "anonymous_client" => "anonymous_submission_rate_limited",
                    "repository_owner" => "repository_owner_rate_limited",
                    _ => "provider_capacity_rate_limited",
                },
                retry_after_seconds: Some(retry_after_seconds),
            },
            AdmissionError::Database(_) => Self::service_unavailable("database_unavailable"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(HostedErrorResponse::new(self.code))).into_response();
        if let Some(seconds) = self.retry_after_seconds
            && let Ok(value) = HeaderValue::from_str(&seconds.max(1).to_string())
        {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_configuration_is_bounded() {
        assert!(bounded_env("ASSAY_TEST_UNSET_BOUND", 8, 1, 10).is_ok());
    }

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
