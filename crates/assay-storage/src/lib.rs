//! PostgreSQL persistence adapter for the hosted Assay runtime.
//!
//! Raw provider payloads and credentials never cross this boundary. Immutable
//! facts are append-only, admission is serialized, and every worker mutation
//! is fenced by the current job generation and lease token.

use std::{error::Error, fmt};

use assay_project_intelligence::{
    HostedAdmission, HostedClaimedJob, HostedContractValueError, HostedEvaluationAttempt,
    HostedEvaluationInput, HostedEvaluationStatus, HostedFailure, HostedFailureScope,
    HostedJobStage, HostedJobState, HostedPortError, HostedProjectStatus,
    HostedProjectStatusRecord, HostedRecentSourceStatus, HostedRecentSourceStatusRecord,
    HostedRequestState, HostedScoreStatus, HostedSourceCollection, HostedStoredSource,
    HostedSubmission, HostedWorkflowPolicy, HostedWorkflowStage, HostedWorkflowStore,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
const MIGRATION_LOCK_ID: i64 = 0x4153_5341_5944_4231;
const ADMISSION_LOCK_ID: i64 = 0x4153_5341_5941_444d;

#[derive(Clone)]
pub struct Storage {
    pool: PgPool,
}

#[derive(Debug)]
pub enum AdmissionError {
    Database(sqlx::Error),
    CapacityFull,
    RateLimited {
        scope: &'static str,
        retry_after_seconds: i64,
    },
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("admission storage unavailable"),
            Self::CapacityFull => formatter.write_str("analysis capacity is full"),
            Self::RateLimited { scope, .. } => {
                write!(formatter, "{scope} admission is cooling down")
            }
        }
    }
}

impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::CapacityFull | Self::RateLimited { .. } => None,
        }
    }
}

impl From<sqlx::Error> for AdmissionError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Debug)]
pub enum StorageError {
    Database(sqlx::Error),
    LeaseLost,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("hosted storage unavailable"),
            Self::LeaseLost => formatter.write_str("job lease was reclaimed"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::LeaseLost => None,
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PublicAdmissionLimits {
    pub max_active: i64,
    pub completed_cooldown_seconds: i64,
    pub failure_backoff_seconds: i64,
    pub bucket_window_seconds: i64,
    pub bucket_cooldown_seconds: i64,
    pub anonymous_burst: i32,
    pub owner_burst: i32,
    pub provider_burst: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct FailureSettlement<'a> {
    pub stage: &'a str,
    pub code: &'a str,
    pub retryable: bool,
    pub requested_delay_seconds: Option<i64>,
    pub retry_delay_cap_seconds: i64,
    pub failure_circuit_threshold: i32,
    pub failure_circuit_cooldown_seconds: i64,
    pub provider_failure: bool,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct ProjectStatusRow {
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
struct RecentSourceStatusRow {
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

#[derive(Clone, Debug)]
pub struct ClaimedJob {
    pub job_id: Uuid,
    pub request_id: Uuid,
    pub owner: String,
    pub repository: String,
    pub generation: i32,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub lease_generation: i64,
    pub lease_token: Uuid,
    pub stage: String,
    pub source_snapshot_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct GitHubCollection {
    pub provider_repository_id: i64,
    pub owner: String,
    pub repository: String,
    pub canonical_url: String,
    pub default_branch: String,
    pub head_sha: String,
    pub source_url: String,
    pub etag: Option<String>,
    pub normalized_facts: Value,
}

#[derive(Clone, Debug)]
pub struct StoredSource {
    pub source_snapshot_id: Uuid,
    pub source_observation_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct StoredEvaluationInput {
    pub source: StoredSource,
    pub normalized_facts: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvaluationAttempt {
    pub provider_id: String,
    pub model: String,
    pub evaluator_profile: String,
    pub rubric_version: String,
    pub prompt_version: String,
    pub evaluation_version: String,
    pub provider_profile_version: String,
    pub sampling: Value,
    pub evidence_bundle_hash: String,
    pub usage: Option<Value>,
    pub latency_ms: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AdmissionJob {
    job_id: Uuid,
    state: String,
    terminal_at: Option<OffsetDateTime>,
    updated_at: OffsetDateTime,
}

#[derive(sqlx::FromRow)]
struct ClaimCandidate {
    job_id: Uuid,
    request_id: Uuid,
    owner: String,
    repository: String,
    generation: i32,
    attempt_count: i32,
    max_attempts: i32,
    lease_generation: i64,
    stage: String,
    source_snapshot_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct AdmissionBucket {
    admitted_count: i32,
    window_started_at: OffsetDateTime,
    blocked_until: Option<OffsetDateTime>,
}

#[derive(sqlx::FromRow)]
struct ReconcileCandidate {
    job_id: Uuid,
    request_id: Uuid,
    generation: i32,
    attempt_count: i32,
    max_attempts: i32,
    stage: String,
    has_reservation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconcileDisposition {
    Retry,
    TerminalRelease,
}

const fn reconcile_disposition(
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

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        let mut connection = self.pool.acquire().await?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *connection)
            .await?;
        let result = MIGRATOR.run(&mut *connection).await;
        let unlock = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *connection)
            .await;
        result?;
        unlock?;
        Ok(())
    }

    pub async fn health(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn submit_seed(
        &self,
        owner: &str,
        repository: &str,
        max_active: i64,
    ) -> Result<HostedSubmission, AdmissionError> {
        self.admit(owner, repository, None, max_active, true).await
    }

    pub async fn submit_public(
        &self,
        owner: &str,
        repository: &str,
        anonymous_bucket_id: &str,
        limits: PublicAdmissionLimits,
    ) -> Result<HostedSubmission, AdmissionError> {
        self.admit(
            owner,
            repository,
            Some((anonymous_bucket_id, limits)),
            limits.max_active,
            false,
        )
        .await
    }

    async fn admit(
        &self,
        owner: &str,
        repository: &str,
        public: Option<(&str, PublicAdmissionLimits)>,
        max_active: i64,
        seed_only: bool,
    ) -> Result<HostedSubmission, AdmissionError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(ADMISSION_LOCK_ID)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"UPDATE analysis_capacity_reservations
                  SET state = 'released', settled_at = now()
                WHERE state = 'reserved' AND expires_at <= now()"#,
        )
        .execute(&mut *tx)
        .await?;
        let owner_bucket_id = sha256_hex(owner.to_ascii_lowercase().as_bytes());
        let anonymous_bucket_id = public.map(|(bucket, _)| bucket);
        let inserted_request: Option<Uuid> = sqlx::query_scalar(
            r#"INSERT INTO source_requests
                 (provider, requested_owner, requested_name, admission_source,
                  anonymous_bucket_id, owner_bucket_id)
               VALUES ('github', $1, $2, $3, $4, $5)
               ON CONFLICT (provider, requested_owner, requested_name) DO NOTHING
               RETURNING id"#,
        )
        .bind(owner)
        .bind(repository)
        .bind(if seed_only { "internal" } else { "public" })
        .bind(anonymous_bucket_id)
        .bind((!seed_only).then_some(&owner_bucket_id))
        .fetch_optional(&mut *tx)
        .await?;
        let request_id = if let Some(id) = inserted_request {
            id
        } else {
            sqlx::query_scalar(
                "SELECT id FROM source_requests WHERE provider = 'github' AND requested_owner = $1 AND requested_name = $2",
            )
            .bind(owner)
            .bind(repository)
            .fetch_one(&mut *tx)
            .await?
        };
        let existing = sqlx::query_as::<_, AdmissionJob>(
            r#"SELECT id AS job_id, state, terminal_at, updated_at
                 FROM analysis_jobs
                WHERE source_request_id = $1 AND evaluator_profile = 'ollama-compatible-1'
                FOR UPDATE"#,
        )
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(job) = &existing {
            if seed_only || matches!(job.state.as_str(), "queued" | "running") {
                tx.commit().await?;
                return Ok(submission(
                    request_id,
                    owner,
                    repository,
                    &job.state,
                    if matches!(job.state.as_str(), "queued" | "running") {
                        "joined_active"
                    } else {
                        "cooldown"
                    },
                    None,
                ));
            }
            let limits = public.map(|(_, limits)| limits);
            let cooldown = if job.state == "complete" {
                limits.map_or(0, |limits| limits.completed_cooldown_seconds)
            } else {
                limits.map_or(0, |limits| limits.failure_backoff_seconds)
            };
            let terminal_at = job.terminal_at.unwrap_or(job.updated_at);
            let eligible_at = terminal_at + Duration::seconds(cooldown.max(0));
            let now = OffsetDateTime::now_utc();
            if eligible_at > now {
                let remaining = (eligible_at - now).whole_seconds().max(1);
                tx.commit().await?;
                return Ok(submission(
                    request_id,
                    owner,
                    repository,
                    &job.state,
                    "cooldown",
                    Some(remaining),
                ));
            }
        }

        if let Some((anonymous_bucket_id, limits)) = public {
            for (kind, bucket_id, burst) in [
                (
                    "anonymous_client",
                    anonymous_bucket_id,
                    limits.anonymous_burst,
                ),
                (
                    "repository_owner",
                    owner_bucket_id.as_str(),
                    limits.owner_burst,
                ),
                ("provider", "github-ollama", limits.provider_burst),
            ] {
                if let Some(retry_after_seconds) = consume_admission_bucket(
                    &mut tx,
                    kind,
                    bucket_id,
                    burst,
                    limits.bucket_window_seconds,
                    limits.bucket_cooldown_seconds,
                )
                .await?
                {
                    if inserted_request.is_some() {
                        sqlx::query(
                            "DELETE FROM source_requests WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM analysis_jobs WHERE source_request_id = $1)",
                        )
                        .bind(request_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                    tx.commit().await?;
                    return Err(AdmissionError::RateLimited {
                        scope: kind,
                        retry_after_seconds,
                    });
                }
            }
        }

        reserve_capacity(&mut tx, max_active).await?;
        let (job_id, generation) = if let Some(job) = existing {
            let generation: i32 = sqlx::query_scalar(
                r#"UPDATE analysis_jobs SET generation = generation + 1,
                           state = 'queued', stage = 'canonicalizing', attempt_count = 0,
                           source_snapshot_id = NULL,
                          next_attempt_at = now(), lease_owner = NULL, lease_token = NULL,
                          lease_expires_at = NULL, last_error_code = NULL, terminal_at = NULL,
                          updated_at = now()
                    WHERE id = $1 RETURNING generation"#,
            )
            .bind(job.job_id)
            .fetch_one(&mut *tx)
            .await?;
            (job.job_id, generation)
        } else {
            sqlx::query_as::<_, (Uuid, i32)>(
                r#"INSERT INTO analysis_jobs (source_request_id)
                   VALUES ($1) RETURNING id, generation"#,
            )
            .bind(request_id)
            .fetch_one(&mut *tx)
            .await?
        };
        sqlx::query(
            r#"INSERT INTO analysis_capacity_reservations (job_id, generation)
               VALUES ($1, $2)"#,
        )
        .bind(job_id)
        .bind(generation)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"UPDATE source_requests SET state = 'queued',
                      admission_source = $3,
                      anonymous_bucket_id = COALESCE($4, anonymous_bucket_id),
                      owner_bucket_id = COALESCE($5, owner_bucket_id),
                      last_public_admitted_at = CASE WHEN $2 THEN last_public_admitted_at ELSE now() END,
                      updated_at = now() WHERE id = $1"#,
        )
        .bind(request_id)
        .bind(seed_only)
        .bind(if seed_only { "internal" } else { "public" })
        .bind(anonymous_bucket_id)
        .bind((!seed_only).then_some(&owner_bucket_id))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(submission(
            request_id, owner, repository, "queued", "admitted", None,
        ))
    }

    async fn status_by_request(&self, id: Uuid) -> Result<Option<ProjectStatusRow>, sqlx::Error> {
        self.fetch_status("sr.id = $1", Some(id), None, None).await
    }

    pub async fn hosted_status_by_request(
        &self,
        id: Uuid,
    ) -> Result<Option<HostedProjectStatus>, sqlx::Error> {
        self.status_by_request(id)
            .await?
            .map(hosted_project_status)
            .transpose()
            .map_err(invalid_contract_state)
    }

    async fn status_by_repository(
        &self,
        owner: &str,
        repository: &str,
    ) -> Result<Option<ProjectStatusRow>, sqlx::Error> {
        self.fetch_status(
            "((sr.requested_owner = $2 AND sr.requested_name = $3) OR (gr.canonical_owner = $2 AND gr.canonical_name = $3))",
            None,
            Some(owner),
            Some(repository),
        )
        .await
    }

    pub async fn hosted_status_by_repository(
        &self,
        owner: &str,
        repository: &str,
    ) -> Result<Option<HostedProjectStatus>, sqlx::Error> {
        self.status_by_repository(owner, repository)
            .await?
            .map(hosted_project_status)
            .transpose()
            .map_err(invalid_contract_state)
    }

    async fn fetch_status(
        &self,
        predicate: &str,
        id: Option<Uuid>,
        owner: Option<&str>,
        repository: Option<&str>,
    ) -> Result<Option<ProjectStatusRow>, sqlx::Error> {
        let query = format!(
            r#"SELECT sr.id AS request_id,
                      COALESCE(gr.canonical_owner, sr.requested_owner) AS owner,
                      COALESCE(gr.canonical_name, sr.requested_name) AS repository,
                      COALESCE(gr.canonical_url,
                        'https://github.com/' || sr.requested_owner || '/' || sr.requested_name) AS canonical_url,
                      sr.state AS request_state, aj.stage AS job_stage, aj.state AS job_state,
                      aj.last_error_code, gr.provider_repository_id,
                      ss.default_branch, ss.commit_sha AS head_sha,
                      NULLIF(go.normalized_facts ->> 'description', '') AS description,
                      (go.normalized_facts ->> 'stargazers_count')::bigint AS stars,
                      es.status AS evaluation_status,
                      COALESCE(cp.score_status, 'pending') AS score_status,
                      aj.next_attempt_at,
                      GREATEST(sr.updated_at, aj.updated_at, COALESCE(cp.updated_at, sr.updated_at)) AS updated_at
                 FROM source_requests sr
                 JOIN analysis_jobs aj ON aj.source_request_id = sr.id
                 LEFT JOIN github_repositories gr ON gr.provider_repository_id = sr.repository_id
                 LEFT JOIN hosted_source_status cp ON cp.provider_repository_id = gr.provider_repository_id
                 LEFT JOIN source_snapshots ss ON ss.id = cp.latest_source_snapshot_id
                 LEFT JOIN github_observations go ON go.id = ss.metadata_observation_id
                 LEFT JOIN evaluation_snapshots es ON es.id = cp.latest_evaluation_snapshot_id
                WHERE {predicate}
                ORDER BY aj.created_at DESC LIMIT 1"#
        );
        let mut query = sqlx::query_as::<_, ProjectStatusRow>(&query);
        if let Some(id) = id {
            query = query.bind(id);
        } else {
            query = query.bind(Uuid::nil()).bind(owner).bind(repository);
        }
        query.fetch_optional(&self.pool).await
    }

    async fn recent_source_statuses(
        &self,
        limit: i64,
    ) -> Result<Vec<RecentSourceStatusRow>, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT gr.canonical_owner AS owner, gr.canonical_name AS repository,
                      gr.canonical_url, gr.provider_repository_id,
                      NULLIF(go.normalized_facts ->> 'description', '') AS description,
                      (go.normalized_facts ->> 'stargazers_count')::bigint AS stars,
                      ss.default_branch, ss.commit_sha AS head_sha,
                      sr.state AS collection_status, es.status AS evaluation_status,
                      cp.score_status, cp.updated_at
                 FROM hosted_source_status cp
                 JOIN github_repositories gr USING (provider_repository_id)
                 JOIN LATERAL (
                   SELECT state FROM source_requests
                    WHERE repository_id = gr.provider_repository_id
                    ORDER BY updated_at DESC LIMIT 1
                 ) sr ON true
                 LEFT JOIN source_snapshots ss ON ss.id = cp.latest_source_snapshot_id
                 LEFT JOIN github_observations go ON go.id = ss.metadata_observation_id
                 LEFT JOIN evaluation_snapshots es ON es.id = cp.latest_evaluation_snapshot_id
                ORDER BY cp.updated_at DESC LIMIT $1"#,
        )
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await
    }

    pub async fn hosted_recent_source_statuses(
        &self,
        limit: i64,
    ) -> Result<Vec<HostedRecentSourceStatus>, sqlx::Error> {
        self.recent_source_statuses(limit)
            .await?
            .into_iter()
            .map(hosted_recent_source_status)
            .collect::<Result<Vec<_>, _>>()
            .map_err(invalid_contract_state)
    }

    pub async fn claim_job(&self, worker: &str) -> Result<Option<ClaimedJob>, StorageError> {
        let mut tx = self.pool.begin().await?;
        reconcile_abandoned(&mut tx).await?;
        let candidate = sqlx::query_as::<_, ClaimCandidate>(
            r#"SELECT aj.id AS job_id, sr.id AS request_id,
                      sr.requested_owner AS owner, sr.requested_name AS repository,
                      aj.generation, aj.attempt_count, aj.max_attempts, aj.lease_generation,
                      aj.stage, aj.source_snapshot_id
                 FROM analysis_jobs aj
                 JOIN source_requests sr ON sr.id = aj.source_request_id
                WHERE aj.state = 'queued' AND aj.next_attempt_at <= now()
                  AND aj.attempt_count < aj.max_attempts
                  AND EXISTS (
                    SELECT 1 FROM analysis_capacity_reservations acr
                     WHERE acr.job_id = aj.id AND acr.generation = aj.generation
                       AND acr.state = 'reserved' AND acr.expires_at > now()
                  )
                ORDER BY aj.created_at
                FOR UPDATE OF aj SKIP LOCKED LIMIT 1"#,
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(candidate) = candidate else {
            tx.commit().await?;
            return Ok(None);
        };
        let lease_token = Uuid::new_v4();
        let lease_generation = candidate.lease_generation + 1;
        let attempt_count = candidate.attempt_count + 1;
        sqlx::query(
            r#"UPDATE analysis_jobs SET state = 'running',
                      stage = CASE WHEN stage = 'evaluating' THEN 'evaluating' ELSE 'collecting' END,
                      attempt_count = $2, lease_owner = $3, lease_generation = $4,
                      lease_token = $5, lease_expires_at = now() + interval '5 minutes',
                      updated_at = now()
                WHERE id = $1 AND generation = $6"#,
        )
        .bind(candidate.job_id)
        .bind(attempt_count)
        .bind(worker)
        .bind(lease_generation)
        .bind(lease_token)
        .bind(candidate.generation)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE source_requests SET state = $2, updated_at = now() WHERE id = $1")
            .bind(candidate.request_id)
            .bind(if candidate.stage == "evaluating" {
                "partial"
            } else {
                "collecting"
            })
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"UPDATE analysis_capacity_reservations SET expires_at = now() + interval '1 hour'
                WHERE job_id = $1 AND generation = $2 AND state = 'reserved'"#,
        )
        .bind(candidate.job_id)
        .bind(candidate.generation)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(ClaimedJob {
            job_id: candidate.job_id,
            request_id: candidate.request_id,
            owner: candidate.owner,
            repository: candidate.repository,
            generation: candidate.generation,
            attempt_count,
            max_attempts: candidate.max_attempts,
            lease_generation,
            lease_token,
            stage: if candidate.stage == "evaluating" {
                "evaluating"
            } else {
                "collecting"
            }
            .to_owned(),
            source_snapshot_id: candidate.source_snapshot_id,
        }))
    }

    pub async fn store_github_collection(
        &self,
        job: &ClaimedJob,
        collection: &GitHubCollection,
    ) -> Result<StoredSource, StorageError> {
        let facts = canonical_json(&collection.normalized_facts);
        let content_hash = sha256_hex(facts.as_bytes());
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        sqlx::query(
            r#"INSERT INTO github_repositories
                 (provider_repository_id, canonical_owner, canonical_name, canonical_url)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (provider_repository_id) DO UPDATE SET
                 canonical_owner = EXCLUDED.canonical_owner,
                 canonical_name = EXCLUDED.canonical_name,
                 canonical_url = EXCLUDED.canonical_url,
                 last_seen_at = now()"#,
        )
        .bind(collection.provider_repository_id)
        .bind(&collection.owner)
        .bind(&collection.repository)
        .bind(&collection.canonical_url)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE source_requests SET repository_id = $2, updated_at = now() WHERE id = $1",
        )
        .bind(job.request_id)
        .bind(collection.provider_repository_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO github_observations
                 (provider_repository_id, observation_kind, source_url, etag, content_hash, normalized_facts)
               VALUES ($1, 'repository_metadata', $2, $3, $4, $5)
               ON CONFLICT (provider_repository_id, observation_kind, content_hash) DO NOTHING"#,
        )
        .bind(collection.provider_repository_id)
        .bind(&collection.source_url)
        .bind(&collection.etag)
        .bind(&content_hash)
        .bind(&collection.normalized_facts)
        .execute(&mut *tx)
        .await?;
        let observation_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM github_observations
                WHERE provider_repository_id = $1
                  AND observation_kind = 'repository_metadata' AND content_hash = $2"#,
        )
        .bind(collection.provider_repository_id)
        .bind(&content_hash)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO source_snapshots
                 (provider_repository_id, commit_sha, default_branch, metadata_observation_id)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (provider_repository_id, commit_sha, metadata_observation_id) DO NOTHING"#,
        )
        .bind(collection.provider_repository_id)
        .bind(&collection.head_sha)
        .bind(&collection.default_branch)
        .bind(observation_id)
        .execute(&mut *tx)
        .await?;
        let source_snapshot_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM source_snapshots
                WHERE provider_repository_id = $1 AND commit_sha = $2
                  AND metadata_observation_id = $3"#,
        )
        .bind(collection.provider_repository_id)
        .bind(&collection.head_sha)
        .bind(observation_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO hosted_source_status (provider_repository_id, latest_source_snapshot_id)
               VALUES ($1, $2)
               ON CONFLICT (provider_repository_id) DO UPDATE SET
                 latest_source_snapshot_id = EXCLUDED.latest_source_snapshot_id,
                 latest_evaluation_snapshot_id = NULL,
                 score_status = 'pending', updated_at = now()"#,
        )
        .bind(collection.provider_repository_id)
        .bind(source_snapshot_id)
        .execute(&mut *tx)
        .await?;
        record_stage(
            &mut tx,
            job,
            "collecting",
            "complete",
            None,
            None,
            Some(source_snapshot_id),
        )
        .await?;
        sqlx::query(
            "UPDATE analysis_jobs SET stage = 'evaluating', source_snapshot_id = $3, updated_at = now() WHERE id = $1 AND generation = $2",
        )
        .bind(job.job_id)
        .bind(job.generation)
        .bind(source_snapshot_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(StoredSource {
            source_snapshot_id,
            source_observation_id: observation_id,
        })
    }

    pub async fn load_evaluation_input(
        &self,
        job: &ClaimedJob,
    ) -> Result<Option<StoredEvaluationInput>, StorageError> {
        let Some(source_snapshot_id) = job.source_snapshot_id else {
            return Ok(None);
        };
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        let row = sqlx::query_as::<_, (Uuid, Value)>(
            r#"SELECT ss.metadata_observation_id, go.normalized_facts
                 FROM source_snapshots ss
                 JOIN github_observations go ON go.id = ss.metadata_observation_id
                WHERE ss.id = $1"#,
        )
        .bind(source_snapshot_id)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(row.map(
            |(source_observation_id, normalized_facts)| StoredEvaluationInput {
                source: StoredSource {
                    source_snapshot_id,
                    source_observation_id,
                },
                normalized_facts,
            },
        ))
    }

    pub async fn store_evaluation(
        &self,
        job: &ClaimedJob,
        source: &StoredSource,
        evaluation: &EvaluationAttempt,
    ) -> Result<Uuid, StorageError> {
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        let id = insert_evaluation_attempt(&mut tx, job, source, evaluation).await?;
        sqlx::query(
            r#"UPDATE hosted_source_status SET latest_evaluation_snapshot_id = $2,
                      score_status = 'unavailable', updated_at = now()
                WHERE latest_source_snapshot_id = $1"#,
        )
        .bind(source.source_snapshot_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let validated = evaluation.status == "validated_unpublished";
        record_stage(
            &mut tx,
            job,
            "evaluating",
            if validated { "complete" } else { "unavailable" },
            evaluation.error_code.as_deref(),
            None,
            Some(id),
        )
        .await?;
        let terminal = if validated { "complete" } else { "partial" };
        sqlx::query(
            r#"UPDATE analysis_jobs SET state = $3, stage = 'evaluating',
                      lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                      last_error_code = $4, terminal_at = now(), updated_at = now()
                WHERE id = $1 AND generation = $2"#,
        )
        .bind(job.job_id)
        .bind(job.generation)
        .bind(terminal)
        .bind(&evaluation.error_code)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE source_requests SET state = $2, updated_at = now() WHERE id = $1")
            .bind(job.request_id)
            .bind(terminal)
            .execute(&mut *tx)
            .await?;
        settle_reservation(
            &mut tx,
            job,
            if validated { "consumed" } else { "released" },
        )
        .await?;
        if validated {
            clear_failure_circuits(&mut tx, job.request_id).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn record_evaluation_attempt(
        &self,
        job: &ClaimedJob,
        source: &StoredSource,
        evaluation: &EvaluationAttempt,
    ) -> Result<Uuid, StorageError> {
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        let id = insert_evaluation_attempt(&mut tx, job, source, evaluation).await?;
        sqlx::query(
            r#"UPDATE hosted_source_status SET latest_evaluation_snapshot_id = $2,
                      score_status = 'unavailable', updated_at = now()
                WHERE latest_source_snapshot_id = $1"#,
        )
        .bind(source.source_snapshot_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn settle_failure(
        &self,
        job: &ClaimedJob,
        failure: FailureSettlement<'_>,
    ) -> Result<bool, StorageError> {
        let backoff_seconds = retry_backoff_seconds(
            job.attempt_count,
            job.max_attempts,
            failure.retryable,
            failure.requested_delay_seconds,
            failure.retry_delay_cap_seconds,
        );
        let retry = backoff_seconds.is_some();
        let mut tx = self.pool.begin().await?;
        let affected = sqlx::query(
            r#"UPDATE analysis_jobs SET state = CASE WHEN $6 THEN 'queued' ELSE 'unavailable' END,
                      stage = $5, next_attempt_at = CASE WHEN $6
                        THEN now() + make_interval(secs => $7) ELSE next_attempt_at END,
                      lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                      last_error_code = $8,
                      terminal_at = CASE WHEN $6 THEN NULL ELSE now() END, updated_at = now()
                WHERE id = $1 AND generation = $2 AND lease_generation = $3
                  AND lease_token = $4 AND state = 'running' AND lease_expires_at > now()"#,
        )
        .bind(job.job_id)
        .bind(job.generation)
        .bind(job.lease_generation)
        .bind(job.lease_token)
        .bind(failure.stage)
        .bind(retry)
        .bind(backoff_seconds.unwrap_or(0))
        .bind(failure.code)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if affected != 1 {
            tx.rollback().await?;
            return Err(StorageError::LeaseLost);
        }
        record_stage(
            &mut tx,
            job,
            failure.stage,
            if retry { "partial" } else { "unavailable" },
            Some(failure.code),
            failure.requested_delay_seconds,
            None,
        )
        .await?;
        record_failure_circuits(
            &mut tx,
            job.request_id,
            failure.failure_circuit_threshold,
            failure.failure_circuit_cooldown_seconds,
            failure.provider_failure,
        )
        .await?;
        sqlx::query("UPDATE source_requests SET state = $2, updated_at = now() WHERE id = $1")
            .bind(job.request_id)
            .bind(if retry { "partial" } else { "unavailable" })
            .execute(&mut *tx)
            .await?;
        if retry {
            sqlx::query(
                r#"UPDATE analysis_capacity_reservations
                      SET expires_at = now() + make_interval(secs => GREATEST($3 + 300, 3600))
                    WHERE job_id = $1 AND generation = $2 AND state = 'reserved'"#,
            )
            .bind(job.job_id)
            .bind(job.generation)
            .bind(backoff_seconds.unwrap_or(0))
            .execute(&mut *tx)
            .await?;
        } else {
            settle_reservation(&mut tx, job, "released").await?;
        }
        tx.commit().await?;
        Ok(retry)
    }
}

async fn insert_evaluation_attempt(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ClaimedJob,
    source: &StoredSource,
    evaluation: &EvaluationAttempt,
) -> Result<Uuid, sqlx::Error> {
    let content = canonical_json(&serde_json::json!({
        "job_id": job.job_id,
        "job_generation": job.generation,
        "attempt_number": job.attempt_count,
        "provider_id": evaluation.provider_id,
        "model": evaluation.model,
        "profile": evaluation.evaluator_profile,
        "rubric_version": evaluation.rubric_version,
        "prompt_version": evaluation.prompt_version,
        "evaluation_version": evaluation.evaluation_version,
        "provider_profile_version": evaluation.provider_profile_version,
        "sampling": evaluation.sampling,
        "evidence_bundle_hash": evaluation.evidence_bundle_hash,
        "usage": evaluation.usage,
        "latency_ms": evaluation.latency_ms,
        "source_observation_id": source.source_observation_id,
        "status": evaluation.status,
        "error_code": evaluation.error_code,
        "judgment": null,
        "score_status": "unavailable"
    }));
    let content_hash = sha256_hex(content.as_bytes());
    sqlx::query(
        r#"INSERT INTO evaluation_snapshots
              (job_id, job_generation, attempt_number, source_snapshot_id,
               provider_id, model, evaluator_profile, rubric_version,
               prompt_version, evaluation_version, provider_profile_version, sampling,
               evidence_bundle_hash, usage, latency_ms, source_observation_id,
               status, error_code, judgment, score_status, content_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, NULL, 'unavailable', $19)
            ON CONFLICT (source_snapshot_id, provider_id, model, evaluator_profile, rubric_version, content_hash)
            DO NOTHING"#,
    )
    .bind(job.job_id)
    .bind(job.generation)
    .bind(job.attempt_count)
    .bind(source.source_snapshot_id)
    .bind(&evaluation.provider_id)
    .bind(&evaluation.model)
    .bind(&evaluation.evaluator_profile)
    .bind(&evaluation.rubric_version)
    .bind(&evaluation.prompt_version)
    .bind(&evaluation.evaluation_version)
    .bind(&evaluation.provider_profile_version)
    .bind(&evaluation.sampling)
    .bind(&evaluation.evidence_bundle_hash)
    .bind(&evaluation.usage)
    .bind(evaluation.latency_ms)
    .bind(source.source_observation_id)
    .bind(&evaluation.status)
    .bind(&evaluation.error_code)
    .bind(&content_hash)
    .execute(&mut **tx)
    .await?;
    sqlx::query_scalar(
        r#"SELECT id FROM evaluation_snapshots
            WHERE source_snapshot_id = $1 AND provider_id = $2 AND model = $3
              AND evaluator_profile = $4 AND rubric_version = $5 AND content_hash = $6"#,
    )
    .bind(source.source_snapshot_id)
    .bind(&evaluation.provider_id)
    .bind(&evaluation.model)
    .bind(&evaluation.evaluator_profile)
    .bind(&evaluation.rubric_version)
    .bind(&content_hash)
    .fetch_one(&mut **tx)
    .await
}

fn submission(
    request_id: Uuid,
    owner: &str,
    repository: &str,
    state: &str,
    admission: &str,
    retry_after_seconds: Option<i64>,
) -> HostedSubmission {
    HostedSubmission::new(
        request_id,
        owner.to_owned(),
        repository.to_owned(),
        HostedRequestState::try_from(state).expect("storage emits a known request state"),
        HostedAdmission::try_from(admission).expect("storage emits a known admission state"),
        retry_after_seconds,
    )
}

fn hosted_project_status(
    value: ProjectStatusRow,
) -> Result<HostedProjectStatus, HostedContractValueError> {
    Ok(HostedProjectStatus::project(HostedProjectStatusRecord {
        request_id: value.request_id,
        owner: value.owner,
        repository: value.repository,
        canonical_url: value.canonical_url,
        request_state: HostedRequestState::try_from(value.request_state.as_str())?,
        job_stage: HostedJobStage::try_from(value.job_stage.as_str())?,
        job_state: HostedJobState::try_from(value.job_state.as_str())?,
        last_error_code: value.last_error_code,
        provider_repository_id: value.provider_repository_id,
        default_branch: value.default_branch,
        head_sha: value.head_sha,
        description: value.description,
        stars: value.stars,
        evaluation_status: value
            .evaluation_status
            .as_deref()
            .map(HostedEvaluationStatus::try_from)
            .transpose()?,
        score_status: HostedScoreStatus::try_from(value.score_status.as_str())?,
        next_attempt_at: value.next_attempt_at,
        updated_at: value.updated_at,
    }))
}

fn hosted_recent_source_status(
    value: RecentSourceStatusRow,
) -> Result<HostedRecentSourceStatus, HostedContractValueError> {
    Ok(HostedRecentSourceStatus::recent_source(
        HostedRecentSourceStatusRecord {
            owner: value.owner,
            repository: value.repository,
            canonical_url: value.canonical_url,
            provider_repository_id: value.provider_repository_id,
            description: value.description,
            stars: value.stars,
            default_branch: value.default_branch,
            head_sha: value.head_sha,
            collection_status: HostedRequestState::try_from(value.collection_status.as_str())?,
            evaluation_status: value
                .evaluation_status
                .as_deref()
                .map(HostedEvaluationStatus::try_from)
                .transpose()?,
            score_status: HostedScoreStatus::try_from(value.score_status.as_str())?,
            updated_at: value.updated_at,
        },
    ))
}

fn invalid_contract_state(_: HostedContractValueError) -> sqlx::Error {
    sqlx::Error::Protocol("hosted persistence state violated the machine contract".to_owned())
}

impl HostedWorkflowStore for Storage {
    async fn claim_job(&self, worker: &str) -> Result<Option<HostedClaimedJob>, HostedPortError> {
        Storage::claim_job(self, worker)
            .await
            .map(|job| job.map(workflow_job))
            .map_err(workflow_error)
    }

    async fn load_evaluation_input(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<Option<HostedEvaluationInput>, HostedPortError> {
        Storage::load_evaluation_input(self, &storage_job(job))
            .await
            .map(|input| {
                input.map(|input| HostedEvaluationInput {
                    source: workflow_source(&input.source),
                    normalized_facts: input.normalized_facts,
                })
            })
            .map_err(workflow_error)
    }

    async fn store_source_collection(
        &self,
        job: &HostedClaimedJob,
        collection: &HostedSourceCollection,
    ) -> Result<HostedStoredSource, HostedPortError> {
        let collection = GitHubCollection {
            provider_repository_id: collection.provider_repository_id,
            owner: collection.owner.clone(),
            repository: collection.repository.clone(),
            canonical_url: collection.canonical_url.clone(),
            default_branch: collection.default_branch.clone(),
            head_sha: collection.head_sha.clone(),
            source_url: collection.source_url.clone(),
            etag: collection.etag.clone(),
            normalized_facts: collection.normalized_facts.clone(),
        };
        Storage::store_github_collection(self, &storage_job(job), &collection)
            .await
            .map(|source| workflow_source(&source))
            .map_err(workflow_error)
    }

    async fn record_evaluation_attempt(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError> {
        Storage::record_evaluation_attempt(
            self,
            &storage_job(job),
            &storage_source(source),
            &storage_attempt(attempt),
        )
        .await
        .map(|_| ())
        .map_err(workflow_error)
    }

    async fn store_evaluation(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError> {
        Storage::store_evaluation(
            self,
            &storage_job(job),
            &storage_source(source),
            &storage_attempt(attempt),
        )
        .await
        .map(|_| ())
        .map_err(workflow_error)
    }

    async fn settle_failure(
        &self,
        job: &HostedClaimedJob,
        stage: HostedWorkflowStage,
        failure: &HostedFailure,
        policy: HostedWorkflowPolicy,
    ) -> Result<bool, HostedPortError> {
        Storage::settle_failure(
            self,
            &storage_job(job),
            FailureSettlement {
                stage: stage.as_str(),
                code: &failure.code,
                retryable: failure.retryable,
                requested_delay_seconds: failure.retry_after_seconds,
                retry_delay_cap_seconds: policy.retry_delay_cap_seconds,
                failure_circuit_threshold: policy.failure_circuit_threshold,
                failure_circuit_cooldown_seconds: policy.failure_circuit_cooldown_seconds,
                provider_failure: failure.scope == HostedFailureScope::Provider,
            },
        )
        .await
        .map_err(workflow_error)
    }
}

fn workflow_error(error: StorageError) -> HostedPortError {
    match error {
        StorageError::LeaseLost => HostedPortError::lease_lost(),
        StorageError::Database(_) => HostedPortError::unavailable(),
    }
}

fn workflow_job(job: ClaimedJob) -> HostedClaimedJob {
    HostedClaimedJob {
        job_id: job.job_id,
        request_id: job.request_id,
        owner: job.owner,
        repository: job.repository,
        generation: job.generation,
        attempt_count: job.attempt_count,
        max_attempts: job.max_attempts,
        lease_generation: job.lease_generation,
        lease_token: job.lease_token,
        stage: if job.stage == "evaluating" {
            HostedWorkflowStage::Evaluating
        } else {
            HostedWorkflowStage::Collecting
        },
        source_snapshot_id: job.source_snapshot_id,
    }
}

fn storage_job(job: &HostedClaimedJob) -> ClaimedJob {
    ClaimedJob {
        job_id: job.job_id,
        request_id: job.request_id,
        owner: job.owner.clone(),
        repository: job.repository.clone(),
        generation: job.generation,
        attempt_count: job.attempt_count,
        max_attempts: job.max_attempts,
        lease_generation: job.lease_generation,
        lease_token: job.lease_token,
        stage: job.stage.as_str().to_owned(),
        source_snapshot_id: job.source_snapshot_id,
    }
}

fn workflow_source(source: &StoredSource) -> HostedStoredSource {
    HostedStoredSource {
        source_snapshot_id: source.source_snapshot_id,
        source_observation_id: source.source_observation_id,
    }
}

fn storage_source(source: &HostedStoredSource) -> StoredSource {
    StoredSource {
        source_snapshot_id: source.source_snapshot_id,
        source_observation_id: source.source_observation_id,
    }
}

fn storage_attempt(attempt: &HostedEvaluationAttempt) -> EvaluationAttempt {
    EvaluationAttempt {
        provider_id: attempt.provider_id.clone(),
        model: attempt.model.clone(),
        evaluator_profile: attempt.evaluator_profile.clone(),
        rubric_version: attempt.rubric_version.clone(),
        prompt_version: attempt.prompt_version.clone(),
        evaluation_version: attempt.evaluation_version.clone(),
        provider_profile_version: attempt.provider_profile_version.clone(),
        sampling: attempt.sampling.clone(),
        evidence_bundle_hash: attempt.evidence_bundle_hash.clone(),
        usage: attempt.usage.clone(),
        latency_ms: attempt.latency_ms,
        status: attempt.status.clone(),
        error_code: attempt.error_code.clone(),
    }
}

async fn consume_admission_bucket(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: &'static str,
    bucket_id: &str,
    burst: i32,
    window_seconds: i64,
    cooldown_seconds: i64,
) -> Result<Option<i64>, AdmissionError> {
    let now = OffsetDateTime::now_utc();
    let current = sqlx::query_as::<_, AdmissionBucket>(
        r#"SELECT admitted_count, window_started_at, blocked_until
             FROM admission_buckets WHERE bucket_kind = $1 AND bucket_id = $2
             FOR UPDATE"#,
    )
    .bind(kind)
    .bind(bucket_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(current) = &current
        && let Some(blocked_until) = current.blocked_until
        && blocked_until > now
    {
        return Ok(Some((blocked_until - now).whole_seconds().max(1)));
    }
    let active_window = current.as_ref().is_some_and(|bucket| {
        bucket.window_started_at + Duration::seconds(window_seconds.max(1)) > now
    });
    if active_window
        && current
            .as_ref()
            .is_some_and(|bucket| bucket.admitted_count >= burst.max(1))
    {
        sqlx::query(
            r#"UPDATE admission_buckets SET blocked_until = now() + make_interval(secs => $3),
                      updated_at = now() WHERE bucket_kind = $1 AND bucket_id = $2"#,
        )
        .bind(kind)
        .bind(bucket_id)
        .bind(cooldown_seconds.max(1))
        .execute(&mut **tx)
        .await?;
        return Ok(Some(cooldown_seconds.max(1)));
    }
    sqlx::query(
        r#"INSERT INTO admission_buckets
             (bucket_kind, bucket_id, window_started_at, admitted_count, blocked_until)
           VALUES ($1, $2, now(), 1, NULL)
           ON CONFLICT (bucket_kind, bucket_id) DO UPDATE SET
             window_started_at = CASE
               WHEN admission_buckets.window_started_at + make_interval(secs => $3) <= now()
               THEN now() ELSE admission_buckets.window_started_at END,
             admitted_count = CASE
               WHEN admission_buckets.window_started_at + make_interval(secs => $3) <= now()
               THEN 1 ELSE admission_buckets.admitted_count + 1 END,
             blocked_until = NULL, updated_at = now()"#,
    )
    .bind(kind)
    .bind(bucket_id)
    .bind(window_seconds.max(1))
    .execute(&mut **tx)
    .await?;
    Ok(None)
}

async fn record_failure_circuits(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: Uuid,
    threshold: i32,
    cooldown_seconds: i64,
    provider_failure: bool,
) -> Result<(), sqlx::Error> {
    let request = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        r#"SELECT admission_source, anonymous_bucket_id, owner_bucket_id
             FROM source_requests WHERE id = $1"#,
    )
    .bind(request_id)
    .fetch_one(&mut **tx)
    .await?;
    if request.0 != "public" {
        return Ok(());
    }
    for (kind, bucket_id) in
        failure_circuit_buckets(request.1.as_deref(), request.2.as_deref(), provider_failure)
    {
        sqlx::query(
            r#"INSERT INTO admission_buckets
                 (bucket_kind, bucket_id, failure_window_started_at, recent_failure_count)
               VALUES ($1, $2, now(), 1)
               ON CONFLICT (bucket_kind, bucket_id) DO UPDATE SET
                 failure_window_started_at = CASE
                   WHEN admission_buckets.failure_window_started_at + interval '10 minutes' <= now()
                   THEN now() ELSE admission_buckets.failure_window_started_at END,
                 recent_failure_count = CASE
                   WHEN admission_buckets.failure_window_started_at + interval '10 minutes' <= now()
                   THEN 1 ELSE admission_buckets.recent_failure_count + 1 END,
                 blocked_until = CASE
                   WHEN (CASE
                     WHEN admission_buckets.failure_window_started_at + interval '10 minutes' <= now()
                     THEN 1 ELSE admission_buckets.recent_failure_count + 1 END) >= $3
                   THEN GREATEST(COALESCE(admission_buckets.blocked_until, now()),
                                 now() + make_interval(secs => $4))
                   ELSE admission_buckets.blocked_until END,
                 updated_at = now()"#,
        )
        .bind(kind)
        .bind(bucket_id)
        .bind(threshold.max(1))
        .bind(cooldown_seconds.max(1))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn failure_circuit_buckets<'a>(
    anonymous_bucket: Option<&'a str>,
    owner_bucket: Option<&'a str>,
    provider_failure: bool,
) -> Vec<(&'static str, &'a str)> {
    let mut buckets = Vec::with_capacity(3);
    if let Some(bucket) = anonymous_bucket {
        buckets.push(("anonymous_client", bucket));
    }
    if let Some(bucket) = owner_bucket {
        buckets.push(("repository_owner", bucket));
    }
    if provider_failure {
        buckets.push(("provider", "github-ollama"));
    }
    buckets
}

async fn clear_failure_circuits(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request_id: Uuid,
) -> Result<(), sqlx::Error> {
    let request = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT admission_source, anonymous_bucket_id, owner_bucket_id FROM source_requests WHERE id = $1",
    )
    .bind(request_id)
    .fetch_one(&mut **tx)
    .await?;
    if request.0 != "public" {
        return Ok(());
    }
    for (kind, bucket_id) in [
        ("anonymous_client", request.1.as_deref()),
        ("repository_owner", request.2.as_deref()),
        ("provider", Some("github-ollama")),
    ] {
        if let Some(bucket_id) = bucket_id {
            sqlx::query(
                "UPDATE admission_buckets SET recent_failure_count = 0, blocked_until = NULL, updated_at = now() WHERE bucket_kind = $1 AND bucket_id = $2",
            )
            .bind(kind)
            .bind(bucket_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn reserve_capacity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    max_active: i64,
) -> Result<(), AdmissionError> {
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM analysis_capacity_reservations WHERE state = 'reserved' AND expires_at > now()",
    )
    .fetch_one(&mut **tx)
    .await?;
    if active >= max_active.clamp(1, 1_000) {
        return Err(AdmissionError::CapacityFull);
    }
    Ok(())
}

async fn reconcile_abandoned(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE analysis_capacity_reservations
              SET state = 'released', settled_at = now()
            WHERE state = 'reserved' AND expires_at <= now()"#,
    )
    .execute(&mut **tx)
    .await?;
    let candidates = sqlx::query_as::<_, ReconcileCandidate>(
        r#"SELECT aj.id AS job_id, aj.source_request_id AS request_id, aj.generation,
                  aj.attempt_count, aj.max_attempts, aj.stage,
                  EXISTS (SELECT 1 FROM analysis_capacity_reservations acr
                           WHERE acr.job_id = aj.id AND acr.generation = aj.generation
                             AND acr.state = 'reserved' AND acr.expires_at > now()) AS has_reservation
             FROM analysis_jobs aj
            WHERE (aj.state = 'running' AND aj.lease_expires_at <= now())
               OR (aj.state IN ('queued', 'running') AND NOT EXISTS (
                    SELECT 1 FROM analysis_capacity_reservations acr
                     WHERE acr.job_id = aj.id AND acr.generation = aj.generation
                       AND acr.state = 'reserved' AND acr.expires_at > now()))
            FOR UPDATE OF aj SKIP LOCKED"#,
    )
    .fetch_all(&mut **tx)
    .await?;
    for candidate in candidates {
        let disposition = reconcile_disposition(
            candidate.has_reservation,
            candidate.attempt_count,
            candidate.max_attempts,
        );
        let retry = disposition == ReconcileDisposition::Retry;
        let code = if candidate.has_reservation {
            "lease_expired"
        } else {
            "capacity_reservation_expired"
        };
        sqlx::query(
            r#"UPDATE analysis_jobs SET state = CASE WHEN $3 THEN 'queued' ELSE 'unavailable' END,
                      next_attempt_at = CASE WHEN $3 THEN now() + interval '4 seconds' ELSE next_attempt_at END,
                      lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                      last_error_code = $4, terminal_at = CASE WHEN $3 THEN NULL ELSE now() END,
                      updated_at = now() WHERE id = $1 AND generation = $2"#,
        )
        .bind(candidate.job_id)
        .bind(candidate.generation)
        .bind(retry)
        .bind(code)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO job_stage_attempts
                 (job_id, generation, attempt_number, stage, status, error_code)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (job_id, generation, attempt_number, stage) DO NOTHING"#,
        )
        .bind(candidate.job_id)
        .bind(candidate.generation)
        .bind(candidate.attempt_count.max(1))
        .bind(&candidate.stage)
        .bind(if retry { "partial" } else { "unavailable" })
        .bind(code)
        .execute(&mut **tx)
        .await?;
        sqlx::query("UPDATE source_requests SET state = $2, updated_at = now() WHERE id = $1")
            .bind(candidate.request_id)
            .bind(if retry { "partial" } else { "unavailable" })
            .execute(&mut **tx)
            .await?;
        if disposition == ReconcileDisposition::TerminalRelease {
            sqlx::query(
                r#"UPDATE analysis_capacity_reservations
                      SET state = 'released', settled_at = now()
                    WHERE job_id = $1 AND generation = $2 AND state = 'reserved'"#,
            )
            .bind(candidate.job_id)
            .bind(candidate.generation)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn fence_and_renew(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ClaimedJob,
) -> Result<(), StorageError> {
    let affected = sqlx::query(
        r#"UPDATE analysis_jobs SET lease_expires_at = now() + interval '5 minutes', updated_at = now()
            WHERE id = $1 AND generation = $2 AND lease_generation = $3
              AND lease_token = $4 AND state = 'running' AND lease_expires_at > now()"#,
    )
    .bind(job.job_id)
    .bind(job.generation)
    .bind(job.lease_generation)
    .bind(job.lease_token)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if affected != 1 {
        return Err(StorageError::LeaseLost);
    }
    Ok(())
}

async fn record_stage(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ClaimedJob,
    stage: &str,
    status: &str,
    error: Option<&str>,
    provider_retry_after_seconds: Option<i64>,
    snapshot: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO job_stage_attempts
             (job_id, generation, attempt_number, stage, status, error_code,
              provider_retry_after_seconds, snapshot_ref)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           ON CONFLICT (job_id, generation, attempt_number, stage) DO NOTHING"#,
    )
    .bind(job.job_id)
    .bind(job.generation)
    .bind(job.attempt_count)
    .bind(stage)
    .bind(status)
    .bind(error)
    .bind(provider_retry_after_seconds)
    .bind(snapshot)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn settle_reservation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ClaimedJob,
    state: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE analysis_capacity_reservations SET state = $3, settled_at = now()
            WHERE job_id = $1 AND generation = $2 AND state = 'reserved'"#,
    )
    .bind(job.job_id)
    .bind(job.generation)
    .bind(state)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).expect("serializing serde_json::Value cannot fail")
}

fn retry_backoff_seconds(
    attempt_count: i32,
    max_attempts: i32,
    retryable: bool,
    requested_delay_seconds: Option<i64>,
    cap_seconds: i64,
) -> Option<i64> {
    (retryable && attempt_count < max_attempts).then(|| {
        let exponential = i64::from(2_i32.pow(attempt_count.clamp(1, 6) as u32)) * 2;
        exponential
            .max(requested_delay_seconds.unwrap_or(0))
            .min(cap_seconds.max(1))
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
