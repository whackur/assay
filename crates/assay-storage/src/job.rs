use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::{
    ClaimCandidate, ReconcileCandidate, ReconcileDisposition, reconcile_disposition,
};
use crate::storage::Storage;
use crate::types::ClaimedJob;

impl Storage {
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

pub(crate) async fn fence_and_renew(
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

pub(crate) async fn record_stage(
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

pub(crate) async fn settle_reservation(
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
