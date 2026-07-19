use assay_project_intelligence::{ProjectAiAnalysisEnvelope, StoredProjectAiAnalysis};
use uuid::Uuid;

use crate::{error::StorageError, storage::Storage};

#[derive(Clone, Debug)]
pub struct PublicationApproval {
    pub evaluation_snapshot_id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub display_name: String,
}

#[derive(sqlx::FromRow)]
struct PublicationDetails {
    canonical_owner: String,
    canonical_name: String,
    commit_sha: String,
    default_branch: String,
    evaluation_version: String,
    rubric_version: String,
    evidence_bundle_hash: String,
    judgment: serde_json::Value,
}

impl Storage {
    pub async fn approve_public_ai_analysis(
        &self,
        approval: &PublicationApproval,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        let (repository_id, current_source_id): (i64, Option<Uuid>) = sqlx::query_as(
            r#"SELECT ss.provider_repository_id, hss.latest_source_snapshot_id
                 FROM evaluation_snapshots es
                 JOIN source_snapshots ss ON ss.id = es.source_snapshot_id
                 JOIN hosted_source_status hss ON hss.provider_repository_id = ss.provider_repository_id
                WHERE es.id = $1
                  AND hss.latest_evaluation_snapshot_id = es.id
                  AND hss.publication_status = 'hidden'
                  AND hss.publication_approval_id IS NULL
                FOR UPDATE OF hss"#,
        )
        .bind(approval.evaluation_snapshot_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StorageError::PublicationNotFound)?;
        let current_source_id = current_source_id.ok_or(StorageError::PublicationNotFound)?;
        let details: Option<PublicationDetails> = sqlx::query_as(
            r#"SELECT gr.canonical_owner, gr.canonical_name, ss.commit_sha, ss.default_branch,
                      es.evaluation_version, es.rubric_version, es.evidence_bundle_hash, es.judgment
                 FROM evaluation_snapshots es
                 JOIN source_snapshots ss ON ss.id = es.source_snapshot_id
                 JOIN github_repositories gr USING (provider_repository_id)
                WHERE es.id = $1 AND es.source_snapshot_id = $2
                  AND es.status = 'validated_unpublished' AND es.judgment IS NOT NULL"#,
        )
        .bind(approval.evaluation_snapshot_id)
        .bind(current_source_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(PublicationDetails {
            canonical_owner: owner,
            canonical_name: repository,
            commit_sha,
            default_branch,
            evaluation_version,
            rubric_version,
            evidence_bundle_hash,
            judgment,
        }) = details
        else {
            return Err(StorageError::PublicationNotSafe);
        };
        if ProjectAiAnalysisEnvelope::from_stored(StoredProjectAiAnalysis {
            owner,
            repository,
            commit_sha,
            default_branch,
            evaluation_version,
            rubric_version,
            evidence_bundle_hash,
            judgment,
        })
        .is_none()
        {
            return Err(StorageError::PublicationNotSafe);
        }
        let approval_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO evaluation_publication_approvals
                 (evaluation_snapshot_id, source_snapshot_id, approval_kind, issuer, subject, display_name)
               VALUES ($1, $2, 'public_ai_analysis', $3, $4, $5) RETURNING id"#,
        )
        .bind(approval.evaluation_snapshot_id).bind(current_source_id)
        .bind(&approval.issuer).bind(&approval.subject).bind(&approval.display_name)
        .fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE hosted_source_status SET publication_status = 'public', publication_approval_id = $2, updated_at = now() WHERE provider_repository_id = $1 AND latest_source_snapshot_id = $3")
            .bind(repository_id).bind(approval_id).bind(current_source_id)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
