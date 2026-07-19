use serde_json::Value;
use uuid::Uuid;

use crate::error::StorageError;
use crate::job::{fence_and_renew, record_stage};
use crate::storage::Storage;
use crate::types::{ClaimedJob, GitHubCollection, StoredEvaluationInput, StoredSource};
use crate::util::{canonical_json, sha256_hex};

impl Storage {
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
}
