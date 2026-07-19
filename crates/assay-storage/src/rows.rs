use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct ProjectStatusRow {
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub request_state: String,
    pub job_stage: String,
    pub job_state: String,
    pub last_error_code: Option<String>,
    pub provider_repository_id: Option<i64>,
    pub default_branch: Option<String>,
    pub head_sha: Option<String>,
    pub description: Option<String>,
    pub stars: Option<i64>,
    pub evaluation_status: Option<String>,
    pub score_status: String,
    pub next_attempt_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct RecentSourceStatusRow {
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub provider_repository_id: i64,
    pub description: Option<String>,
    pub stars: Option<i64>,
    pub default_branch: Option<String>,
    pub head_sha: Option<String>,
    pub collection_status: String,
    pub evaluation_status: Option<String>,
    pub score_status: String,
    pub updated_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
pub(crate) struct AdmissionJob {
    pub job_id: Uuid,
    pub state: String,
    pub terminal_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
pub(crate) struct ClaimCandidate {
    pub job_id: Uuid,
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub generation: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub lease_generation: i64,
    pub stage: String,
    pub source_snapshot_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct AdmissionBucket {
    pub admitted_count: i32,
    pub window_started_at: OffsetDateTime,
    pub blocked_until: Option<OffsetDateTime>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct ReconcileCandidate {
    pub job_id: Uuid,
    pub request_id: Uuid,
    pub generation: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub stage: String,
    pub has_reservation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReconcileDisposition {
    Retry,
    TerminalRelease,
}

pub(crate) const fn reconcile_disposition(
    has_reservation: bool,
    attempt_count: i32,
    max_attempts: i32,
) -> ReconcileDisposition {
    if has_reservation && attempt_count < max_attempts {
        ReconcileDisposition::Retry
    } else {
        ReconcileDisposition::TerminalRelease
    }
}
