use assay_project_intelligence::HostedScoreArtifact;
use uuid::Uuid;

use crate::error::StorageError;
use crate::failure_circuit::clear_failure_circuits;
use crate::job::{fence_and_renew, record_stage, settle_reservation};
use crate::storage::Storage;
use crate::types::{ClaimedJob, EvaluationAttempt, StoredSource};
use crate::util::{canonical_json, sha256_hex};

impl Storage {
    pub async fn store_evaluation(
        &self,
        job: &ClaimedJob,
        source: &StoredSource,
        evaluation: &EvaluationAttempt,
        score: &HostedScoreArtifact,
    ) -> Result<Uuid, StorageError> {
        validate_evaluation_attempt(evaluation)?;
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        let id = insert_evaluation_attempt(&mut tx, job, source, evaluation, score).await?;
        let compiler_version = score.snapshot["compiler"]["version"]
            .as_str()
            .unwrap_or("unknown");
        let rule_set_hash = score.snapshot["compiler"]["rule_set_hash"]
            .as_str()
            .unwrap_or("unknown");
        let snapshot_hash = sha256_hex(canonical_json(&score.snapshot).as_bytes());
        sqlx::query(
            r#"INSERT INTO hosted_score_snapshots
                 (evaluation_snapshot_id, source_snapshot_id, schema_version,
                  compiler_version, rule_set_hash, content_hash, score_status, score_value, snapshot)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (evaluation_snapshot_id, compiler_version, rule_set_hash) DO NOTHING"#,
        )
        .bind(id)
        .bind(source.source_snapshot_id)
        .bind(score.snapshot["schema_version"].as_str().unwrap_or("unknown"))
        .bind(compiler_version)
        .bind(rule_set_hash)
        .bind(&snapshot_hash)
        .bind(&score.status)
        .bind(score.value)
        .bind(&score.snapshot)
        .execute(&mut *tx)
        .await?;
        let persisted_hash: String = sqlx::query_scalar(
            r#"SELECT content_hash FROM hosted_score_snapshots
                WHERE evaluation_snapshot_id = $1 AND compiler_version = $2
                  AND rule_set_hash = $3"#,
        )
        .bind(id)
        .bind(compiler_version)
        .bind(rule_set_hash)
        .fetch_one(&mut *tx)
        .await?;
        if persisted_hash != snapshot_hash {
            return Err(StorageError::ScoreSnapshotConflict);
        }
        sqlx::query(
            r#"UPDATE hosted_source_status SET latest_evaluation_snapshot_id = $2,
                      publication_status = 'hidden', publication_approval_id = NULL,
                      score_status = $3, updated_at = now()
                WHERE latest_source_snapshot_id = $1"#,
        )
        .bind(source.source_snapshot_id)
        .bind(id)
        .bind(&score.status)
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
        validate_evaluation_attempt(evaluation)?;
        let mut tx = self.pool.begin().await?;
        fence_and_renew(&mut tx, job).await?;
        let unavailable = HostedScoreArtifact {
            status: "unavailable".to_owned(),
            value: None,
            snapshot: serde_json::json!({}),
        };
        let id = insert_evaluation_attempt(&mut tx, job, source, evaluation, &unavailable).await?;
        sqlx::query(
            r#"UPDATE hosted_source_status SET latest_evaluation_snapshot_id = $2,
                      publication_status = 'hidden', publication_approval_id = NULL,
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
}

fn validate_evaluation_attempt(evaluation: &EvaluationAttempt) -> Result<(), StorageError> {
    let is_validated = evaluation.status == "validated_unpublished";
    if is_validated != evaluation.judgment.is_some() {
        return Err(StorageError::InvalidEvaluation);
    }
    Ok(())
}

async fn insert_evaluation_attempt(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ClaimedJob,
    source: &StoredSource,
    evaluation: &EvaluationAttempt,
    score: &HostedScoreArtifact,
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
        "judgment": evaluation.judgment,
        "score_status": score.status
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
                    $14, $15, $16, $17, $18, $19, $20, $21)
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
    .bind(&evaluation.judgment)
    .bind(&score.status)
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
