use crate::error::StorageError;
use crate::failure_circuit::record_failure_circuits;
use crate::job::{record_stage, settle_reservation};
use crate::storage::Storage;
use crate::types::{ClaimedJob, FailureSettlement};

impl Storage {
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

pub(crate) fn retry_backoff_seconds(
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
