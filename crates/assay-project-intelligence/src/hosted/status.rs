use serde::Serialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::enums::{
    HostedEvaluationStatus, HostedJobStage, HostedJobState, HostedRequestState, HostedScoreStatus,
};

#[derive(Clone, Debug)]
pub struct HostedProjectStatusRecord {
    pub request_id: uuid::Uuid,
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
