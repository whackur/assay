use crate::{
    rows::{ProjectAiAnalysisRow, ReviewQueueRow},
    storage::Storage,
};
use assay_project_intelligence::{ProjectAiAnalysisEnvelope, StoredProjectAiAnalysis};
use serde::Serialize;
use uuid::Uuid;

pub const REVIEW_QUEUE_LIMIT: i64 = 50;

#[derive(Clone, Serialize)]
pub struct ReviewQueueItem {
    pub evaluation_snapshot_id: Uuid,
    pub analysis: ProjectAiAnalysisEnvelope,
    pub provenance: ReviewQueueProvenance,
}

#[derive(Clone, Serialize)]
pub struct ReviewQueueProvenance {
    pub source_snapshot_id: Uuid,
    pub source_observation_id: Uuid,
    pub provider_id: String,
    pub model: String,
    pub evaluator_profile: String,
    pub rubric_version: String,
    pub prompt_version: String,
    pub evaluation_version: String,
    pub provider_profile_version: String,
    pub evidence_bundle_hash: String,
    pub content_hash: String,
}

impl Storage {
    pub async fn project_ai_analysis_by_repository(
        &self,
        owner: &str,
        repository: &str,
    ) -> Result<Option<ProjectAiAnalysisEnvelope>, sqlx::Error> {
        let row = sqlx::query_as::<_, ProjectAiAnalysisRow>(
            r#"SELECT gr.canonical_owner AS owner, gr.canonical_name AS repository,
                      ss.commit_sha, ss.default_branch, es.evaluation_version,
                      es.rubric_version, es.evidence_bundle_hash, es.judgment
                 FROM hosted_source_status cp
                 JOIN github_repositories gr USING (provider_repository_id)
                 JOIN source_snapshots ss ON ss.id = cp.latest_source_snapshot_id
                 JOIN evaluation_publication_approvals epa ON epa.id = cp.publication_approval_id
                 JOIN evaluation_snapshots es ON es.id = epa.evaluation_snapshot_id
                  AND es.source_snapshot_id = ss.id
                  AND epa.source_snapshot_id = ss.id
                  AND es.status = 'validated_unpublished' AND es.judgment IS NOT NULL
                WHERE cp.publication_status = 'public'
                  AND gr.canonical_owner = $1 AND gr.canonical_name = $2
                LIMIT 1"#,
        )
        .bind(owner)
        .bind(repository)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|row| {
            ProjectAiAnalysisEnvelope::from_stored(StoredProjectAiAnalysis {
                owner: row.owner,
                repository: row.repository,
                commit_sha: row.commit_sha,
                default_branch: row.default_branch,
                evaluation_version: row.evaluation_version,
                rubric_version: row.rubric_version,
                evidence_bundle_hash: row.evidence_bundle_hash,
                judgment: row.judgment,
            })
        }))
    }

    /// Returns only bounded, current-source judgments that are safe for an
    /// administrator to review. Invalid rows are deliberately omitted.
    pub async fn hosted_ai_analysis_review_queue(
        &self,
    ) -> Result<Vec<ReviewQueueItem>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ReviewQueueRow>(
            r#"SELECT es.id AS evaluation_snapshot_id, ss.id AS source_snapshot_id,
                      es.source_observation_id, gr.canonical_owner AS owner,
                      gr.canonical_name AS repository, ss.commit_sha,
                      ss.default_branch, es.provider_id, es.model,
                      es.evaluator_profile, es.rubric_version, es.prompt_version,
                      es.evaluation_version, es.provider_profile_version,
                      es.evidence_bundle_hash, es.content_hash, es.judgment
                 FROM hosted_source_status hss
                 JOIN github_repositories gr USING (provider_repository_id)
                 JOIN source_snapshots ss ON ss.id = hss.latest_source_snapshot_id
                  AND ss.provider_repository_id = hss.provider_repository_id
                 JOIN evaluation_snapshots es
                   ON es.id = hss.latest_evaluation_snapshot_id
                  AND es.source_snapshot_id = ss.id
                 WHERE hss.publication_status = 'hidden'
                   AND hss.publication_approval_id IS NULL
                   AND es.status = 'validated_unpublished'
                   AND es.judgment IS NOT NULL
                 ORDER BY es.created_at ASC, es.id ASC
                 LIMIT $1"#,
        )
        .bind(REVIEW_QUEUE_LIMIT)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                if !bounded_provenance(&row) {
                    return None;
                }
                let analysis = ProjectAiAnalysisEnvelope::from_stored(StoredProjectAiAnalysis {
                    owner: row.owner,
                    repository: row.repository,
                    commit_sha: row.commit_sha,
                    default_branch: row.default_branch,
                    evaluation_version: row.evaluation_version.clone(),
                    rubric_version: row.rubric_version.clone(),
                    evidence_bundle_hash: row.evidence_bundle_hash.clone(),
                    judgment: row.judgment,
                })?;
                Some(ReviewQueueItem {
                    evaluation_snapshot_id: row.evaluation_snapshot_id,
                    analysis,
                    provenance: ReviewQueueProvenance {
                        source_snapshot_id: row.source_snapshot_id,
                        source_observation_id: row.source_observation_id,
                        provider_id: row.provider_id,
                        model: row.model,
                        evaluator_profile: row.evaluator_profile,
                        rubric_version: row.rubric_version,
                        prompt_version: row.prompt_version,
                        evaluation_version: row.evaluation_version,
                        provider_profile_version: row.provider_profile_version,
                        evidence_bundle_hash: row.evidence_bundle_hash,
                        content_hash: row.content_hash,
                    },
                })
            })
            .collect())
    }
}

fn bounded_provenance(row: &ReviewQueueRow) -> bool {
    [
        &row.provider_id,
        &row.model,
        &row.evaluator_profile,
        &row.rubric_version,
        &row.prompt_version,
        &row.evaluation_version,
        &row.provider_profile_version,
        &row.evidence_bundle_hash,
        &row.content_hash,
    ]
    .into_iter()
    .all(|value| !value.is_empty() && value.len() <= 256)
}
