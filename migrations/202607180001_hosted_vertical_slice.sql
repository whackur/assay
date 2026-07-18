CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE source_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    provider text NOT NULL CHECK (provider = 'github'),
    requested_owner text NOT NULL,
    requested_name text NOT NULL,
    state text NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued', 'collecting', 'partial', 'complete', 'unavailable')),
    repository_id bigint,
    admission_source text NOT NULL DEFAULT 'public'
        CHECK (admission_source IN ('public', 'internal')),
    anonymous_bucket_id text,
    owner_bucket_id text,
    last_public_admitted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider, requested_owner, requested_name)
);

CREATE TABLE github_repositories (
    provider_repository_id bigint PRIMARY KEY,
    canonical_owner text NOT NULL,
    canonical_name text NOT NULL,
    canonical_url text NOT NULL,
    first_seen_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE source_requests
    ADD CONSTRAINT source_requests_repository_fk
    FOREIGN KEY (repository_id) REFERENCES github_repositories(provider_repository_id);

CREATE TABLE github_observations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_repository_id bigint NOT NULL
        REFERENCES github_repositories(provider_repository_id),
    observation_kind text NOT NULL,
    observed_at timestamptz NOT NULL DEFAULT now(),
    source_url text NOT NULL,
    etag text,
    content_hash text NOT NULL,
    normalized_facts jsonb NOT NULL,
    UNIQUE (provider_repository_id, observation_kind, content_hash)
);

COMMENT ON COLUMN github_observations.normalized_facts IS
    'Normalized public facts only; full source blobs, raw diffs, and credential-bearing payloads are forbidden.';

CREATE TABLE source_snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_repository_id bigint NOT NULL
        REFERENCES github_repositories(provider_repository_id),
    commit_sha text NOT NULL CHECK (commit_sha ~ '^[0-9a-f]{40}$'),
    default_branch text NOT NULL,
    metadata_observation_id uuid NOT NULL REFERENCES github_observations(id),
    collected_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider_repository_id, commit_sha, metadata_observation_id)
);

CREATE TABLE analysis_jobs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    source_request_id uuid NOT NULL REFERENCES source_requests(id),
    evaluator_profile text NOT NULL DEFAULT 'ollama-compatible-1',
    generation integer NOT NULL DEFAULT 1 CHECK (generation > 0),
    state text NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued', 'running', 'partial', 'complete', 'unavailable')),
    stage text NOT NULL DEFAULT 'canonicalizing',
    source_snapshot_id uuid REFERENCES source_snapshots(id),
    attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts integer NOT NULL DEFAULT 3 CHECK (max_attempts BETWEEN 1 AND 10),
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    lease_owner text,
    lease_generation bigint NOT NULL DEFAULT 0 CHECK (lease_generation >= 0),
    lease_token uuid,
    lease_expires_at timestamptz,
    last_error_code text,
    terminal_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_request_id, evaluator_profile)
);

CREATE INDEX analysis_jobs_claim_idx
    ON analysis_jobs (state, next_attempt_at, lease_expires_at, created_at);

CREATE TABLE analysis_capacity_reservations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id uuid NOT NULL REFERENCES analysis_jobs(id),
    generation integer NOT NULL,
    state text NOT NULL DEFAULT 'reserved'
        CHECK (state IN ('reserved', 'consumed', 'released')),
    reserved_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT now() + interval '1 hour',
    settled_at timestamptz,
    UNIQUE (job_id, generation)
);

CREATE INDEX analysis_capacity_active_idx
    ON analysis_capacity_reservations (state, expires_at);

CREATE TABLE admission_buckets (
    bucket_kind text NOT NULL CHECK (bucket_kind IN ('anonymous_client', 'repository_owner', 'provider')),
    bucket_id text NOT NULL,
    window_started_at timestamptz NOT NULL DEFAULT now(),
    admitted_count integer NOT NULL DEFAULT 0 CHECK (admitted_count >= 0),
    failure_window_started_at timestamptz NOT NULL DEFAULT now(),
    recent_failure_count integer NOT NULL DEFAULT 0 CHECK (recent_failure_count >= 0),
    blocked_until timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (bucket_kind, bucket_id)
);

CREATE TABLE job_stage_attempts (
    id bigserial PRIMARY KEY,
    job_id uuid NOT NULL REFERENCES analysis_jobs(id),
    generation integer NOT NULL,
    attempt_number integer NOT NULL CHECK (attempt_number > 0),
    stage text NOT NULL,
    status text NOT NULL CHECK (status IN ('complete', 'partial', 'unavailable')),
    error_code text,
    provider_retry_after_seconds bigint,
    snapshot_ref uuid,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (job_id, generation, attempt_number, stage)
);

CREATE TABLE evaluation_snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id uuid NOT NULL REFERENCES analysis_jobs(id),
    job_generation integer NOT NULL,
    attempt_number integer NOT NULL CHECK (attempt_number > 0),
    source_snapshot_id uuid NOT NULL REFERENCES source_snapshots(id),
    provider_id text NOT NULL,
    model text NOT NULL,
    evaluator_profile text NOT NULL,
    rubric_version text NOT NULL,
    prompt_version text NOT NULL,
    evaluation_version text NOT NULL,
    provider_profile_version text NOT NULL,
    sampling jsonb NOT NULL,
    evidence_bundle_hash text NOT NULL,
    usage jsonb,
    latency_ms bigint,
    source_observation_id uuid NOT NULL REFERENCES github_observations(id),
    status text NOT NULL CHECK (status IN ('validated_unpublished', 'partial', 'unavailable')),
    error_code text,
    judgment jsonb CHECK (judgment IS NULL),
    score_status text NOT NULL DEFAULT 'unavailable' CHECK (score_status = 'unavailable'),
    content_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_snapshot_id, provider_id, model, evaluator_profile, rubric_version, content_hash),
    CHECK (status <> 'validated_unpublished' OR error_code IS NULL)
);

CREATE TABLE hosted_source_status (
    provider_repository_id bigint PRIMARY KEY
        REFERENCES github_repositories(provider_repository_id),
    latest_source_snapshot_id uuid REFERENCES source_snapshots(id),
    latest_evaluation_snapshot_id uuid REFERENCES evaluation_snapshots(id),
    publication_status text NOT NULL DEFAULT 'hidden'
        CHECK (publication_status IN ('public', 'hidden')),
    score_status text NOT NULL DEFAULT 'unavailable'
        CHECK (score_status IN ('pending', 'unavailable')),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX github_observations_history_idx
    ON github_observations (provider_repository_id, observed_at DESC);
CREATE INDEX source_snapshots_history_idx
    ON source_snapshots (provider_repository_id, collected_at DESC);
CREATE INDEX evaluation_snapshots_history_idx
    ON evaluation_snapshots (source_snapshot_id, created_at DESC);
