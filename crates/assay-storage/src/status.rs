use assay_project_intelligence::{
    HostedContractValueError, HostedEvaluationStatus, HostedJobStage, HostedJobState,
    HostedProjectStatus, HostedProjectStatusRecord, HostedRecentSourceStatus,
    HostedRecentSourceStatusRecord, HostedRequestState, HostedScoreStatus,
};
use uuid::Uuid;

use crate::rows::{ProjectStatusRow, RecentSourceStatusRow};
use crate::storage::Storage;

impl Storage {
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
                      CASE
                        WHEN es.status = 'validated_unpublished' AND es.judgment IS NULL
                          THEN 'unavailable'
                        WHEN es.status = 'validated_unpublished' AND cp.publication_status = 'public'
                          THEN 'validated_published'
                        ELSE es.status
                      END AS evaluation_status,
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
        let mut query = sqlx::query_as::<_, ProjectStatusRow>(sqlx::AssertSqlSafe(query));
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
                      sr.state AS collection_status,
                      CASE
                        WHEN es.status = 'validated_unpublished' AND es.judgment IS NULL
                          THEN 'unavailable'
                        WHEN es.status = 'validated_unpublished' AND cp.publication_status = 'public'
                          THEN 'validated_published'
                        ELSE es.status
                      END AS evaluation_status,
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
