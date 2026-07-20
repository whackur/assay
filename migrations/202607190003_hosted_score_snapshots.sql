CREATE TABLE hosted_score_snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    evaluation_snapshot_id uuid NOT NULL REFERENCES evaluation_snapshots(id),
    source_snapshot_id uuid NOT NULL REFERENCES source_snapshots(id),
    schema_version text NOT NULL,
    compiler_version text NOT NULL,
    rule_set_hash text NOT NULL,
    content_hash text NOT NULL,
    score_status text NOT NULL CHECK (score_status IN ('complete', 'partial', 'insufficient', 'unavailable')),
    score_value double precision,
    snapshot jsonb NOT NULL CHECK (jsonb_typeof(snapshot) = 'object' AND NOT snapshot::text LIKE '%"rationale"%'),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (evaluation_snapshot_id, compiler_version, rule_set_hash)
);

ALTER TABLE hosted_source_status
    DROP CONSTRAINT hosted_source_status_score_status_check,
    ADD CONSTRAINT hosted_source_status_score_status_check
    CHECK (score_status IN ('pending', 'complete', 'partial', 'insufficient', 'unavailable'));
