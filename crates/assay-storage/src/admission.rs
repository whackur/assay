use assay_project_intelligence::{HostedAdmission, HostedRequestState, HostedSubmission};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::admission_bucket::{consume_admission_bucket, reserve_capacity};
use crate::error::AdmissionError;
use crate::rows::AdmissionJob;
use crate::storage::Storage;
use crate::types::PublicAdmissionLimits;
use crate::util::sha256_hex;

const ADMISSION_LOCK_ID: i64 = 0x4153_5341_5941_444d;

impl Storage {
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
}

pub(crate) fn submission(
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
