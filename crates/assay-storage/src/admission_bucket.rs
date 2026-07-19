use time::{Duration, OffsetDateTime};

use crate::error::AdmissionError;
use crate::rows::AdmissionBucket;

pub(crate) async fn consume_admission_bucket(
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

pub(crate) async fn reserve_capacity(
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
