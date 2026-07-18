//! Hosted source-processing machine contracts and projection policy.
//!
//! These contracts report collection and evaluation workflow state. They do
//! not represent project-catalog publication, a provider judgment, or a score.

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

pub const HOSTED_API_CONTRACT: &str = "assay-hosted-api";
pub const HOSTED_API_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedRequestState {
    Queued,
    Collecting,
    Partial,
    Complete,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedAdmission {
    Admitted,
    JoinedActive,
    Cooldown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedJobStage {
    Canonicalizing,
    Collecting,
    Evaluating,
    Publishing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedJobState {
    Queued,
    Running,
    Partial,
    Complete,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedEvaluationStatus {
    ValidatedUnpublished,
    Partial,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedScoreStatus {
    Pending,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedContractValueError;

macro_rules! storage_enum {
    ($type:ty, { $($value:literal => $variant:path),+ $(,)? }) => {
        impl TryFrom<&str> for $type {
            type Error = HostedContractValueError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok($variant),)+
                    _ => Err(HostedContractValueError),
                }
            }
        }
    };
}

storage_enum!(HostedRequestState, {
    "queued" => HostedRequestState::Queued,
    "collecting" => HostedRequestState::Collecting,
    "running" => HostedRequestState::Collecting,
    "partial" => HostedRequestState::Partial,
    "complete" => HostedRequestState::Complete,
    "unavailable" => HostedRequestState::Unavailable,
});
storage_enum!(HostedAdmission, {
    "admitted" => HostedAdmission::Admitted,
    "joined_active" => HostedAdmission::JoinedActive,
    "cooldown" => HostedAdmission::Cooldown,
});
storage_enum!(HostedJobStage, {
    "canonicalizing" => HostedJobStage::Canonicalizing,
    "collecting" => HostedJobStage::Collecting,
    "evaluating" => HostedJobStage::Evaluating,
    "publishing" => HostedJobStage::Publishing,
});
storage_enum!(HostedJobState, {
    "queued" => HostedJobState::Queued,
    "running" => HostedJobState::Running,
    "partial" => HostedJobState::Partial,
    "complete" => HostedJobState::Complete,
    "unavailable" => HostedJobState::Unavailable,
});
storage_enum!(HostedEvaluationStatus, {
    "validated_unpublished" => HostedEvaluationStatus::ValidatedUnpublished,
    "partial" => HostedEvaluationStatus::Partial,
    "unavailable" => HostedEvaluationStatus::Unavailable,
});
storage_enum!(HostedScoreStatus, {
    "pending" => HostedScoreStatus::Pending,
    "unavailable" => HostedScoreStatus::Unavailable,
});

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedSubmissionRequest {
    pub repository: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedContractEnvelope<T> {
    contract: &'static str,
    schema_version: &'static str,
    data: T,
}

impl<T> HostedContractEnvelope<T> {
    pub const fn new(data: T) -> Self {
        Self {
            contract: HOSTED_API_CONTRACT,
            schema_version: HOSTED_API_SCHEMA_VERSION,
            data,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedErrorResponse {
    contract: &'static str,
    schema_version: &'static str,
    error: HostedError,
}

impl HostedErrorResponse {
    pub const fn new(code: &'static str) -> Self {
        Self {
            contract: HOSTED_API_CONTRACT,
            schema_version: HOSTED_API_SCHEMA_VERSION,
            error: HostedError { code },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct HostedError {
    code: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedSubmission {
    request_id: String,
    owner: String,
    repository: String,
    canonical_url: String,
    state: HostedRequestState,
    admission: HostedAdmission,
    retry_after_seconds: Option<i64>,
}

impl HostedSubmission {
    pub fn new(
        request_id: Uuid,
        owner: String,
        repository: String,
        state: HostedRequestState,
        admission: HostedAdmission,
        retry_after_seconds: Option<i64>,
    ) -> Self {
        let canonical_url = format!("https://github.com/{owner}/{repository}");
        Self {
            request_id: request_id.to_string(),
            owner,
            repository,
            canonical_url,
            state,
            admission,
            retry_after_seconds,
        }
    }

    pub const fn admission(&self) -> HostedAdmission {
        self.admission
    }

    pub const fn state(&self) -> HostedRequestState {
        self.state
    }

    pub const fn retry_after_seconds(&self) -> Option<i64> {
        self.retry_after_seconds
    }
}

#[derive(Clone, Debug)]
pub struct HostedProjectStatusRecord {
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub request_state: HostedRequestState,
    pub job_stage: HostedJobStage,
    pub job_state: HostedJobState,
    pub last_error_code: Option<String>,
    pub provider_repository_id: Option<i64>,
    pub default_branch: Option<String>,
    pub head_sha: Option<String>,
    pub description: Option<String>,
    pub stars: Option<i64>,
    pub evaluation_status: Option<HostedEvaluationStatus>,
    pub score_status: HostedScoreStatus,
    pub next_attempt_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedProjectStatus {
    request_id: String,
    owner: String,
    repository: String,
    canonical_url: String,
    request_state: HostedRequestState,
    job_stage: HostedJobStage,
    job_state: HostedJobState,
    last_error_code: Option<String>,
    provider_repository_id: Option<String>,
    default_branch: Option<String>,
    head_sha: Option<String>,
    description: Option<String>,
    stars: Option<i64>,
    evaluation_status: Option<HostedEvaluationStatus>,
    score_status: HostedScoreStatus,
    next_attempt_at: String,
    updated_at: String,
}

impl HostedProjectStatus {
    pub fn project(record: HostedProjectStatusRecord) -> Self {
        Self {
            request_id: record.request_id.to_string(),
            owner: record.owner,
            repository: record.repository,
            canonical_url: record.canonical_url,
            request_state: record.request_state,
            job_stage: record.job_stage,
            job_state: record.job_state,
            last_error_code: record.last_error_code,
            provider_repository_id: record.provider_repository_id.map(|id| id.to_string()),
            default_branch: record.default_branch,
            head_sha: record.head_sha,
            description: record.description,
            stars: record.stars,
            evaluation_status: record.evaluation_status,
            score_status: record.score_status,
            next_attempt_at: timestamp(record.next_attempt_at),
            updated_at: timestamp(record.updated_at),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostedRecentSourceStatusRecord {
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub provider_repository_id: i64,
    pub description: Option<String>,
    pub stars: Option<i64>,
    pub default_branch: Option<String>,
    pub head_sha: Option<String>,
    pub collection_status: HostedRequestState,
    pub evaluation_status: Option<HostedEvaluationStatus>,
    pub score_status: HostedScoreStatus,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedRecentSourceStatus {
    owner: String,
    repository: String,
    canonical_url: String,
    provider_repository_id: String,
    description: Option<String>,
    stars: Option<i64>,
    default_branch: Option<String>,
    head_sha: Option<String>,
    collection_status: HostedRequestState,
    evaluation_status: Option<HostedEvaluationStatus>,
    score_status: HostedScoreStatus,
    updated_at: String,
}

impl HostedRecentSourceStatus {
    pub fn recent_source(record: HostedRecentSourceStatusRecord) -> Self {
        Self {
            owner: record.owner,
            repository: record.repository,
            canonical_url: record.canonical_url,
            provider_repository_id: record.provider_repository_id.to_string(),
            description: record.description,
            stars: record.stars,
            default_branch: record.default_branch,
            head_sha: record.head_sha,
            collection_status: record.collection_status,
            evaluation_status: record.evaluation_status,
            score_status: record.score_status,
            updated_at: timestamp(record.updated_at),
        }
    }
}

fn timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime always formats as RFC 3339")
}
