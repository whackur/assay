# Database migrations

Store forward PostgreSQL migrations here. Migrations must preserve raw facts
and the reproducibility of versioned analysis runs.

The API serializes startup migrations with a PostgreSQL advisory lock. Hosted
Compose uses a named volume. Normal image upgrades preserve it; **never run
`docker compose -f compose.hosted.yaml down -v` against data you need** because
`-v` permanently removes the PostgreSQL volume.

The hosted schema keeps immutable GitHub observations, exact source snapshots,
and provider-attempt provenance separate from the mutable processing-status
read model. Evaluation-only retries retain their source snapshot and provider
retry timing. Rows in `hosted_source_status` default to hidden and are not a
project-catalog publication decision.
