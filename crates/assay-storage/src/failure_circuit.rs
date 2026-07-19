use uuid::Uuid;

pub(crate) fn failure_circuit_buckets<'a>(
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

pub(crate) async fn record_failure_circuits(
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

pub(crate) async fn clear_failure_circuits(
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
