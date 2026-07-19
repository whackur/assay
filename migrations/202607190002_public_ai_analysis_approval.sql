CREATE TABLE evaluation_publication_approvals (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    evaluation_snapshot_id uuid NOT NULL REFERENCES evaluation_snapshots(id),
    source_snapshot_id uuid NOT NULL REFERENCES source_snapshots(id),
    approval_kind text NOT NULL CHECK (approval_kind = 'public_ai_analysis'),
    issuer text NOT NULL,
    subject text NOT NULL,
    display_name text NOT NULL,
    approved_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (evaluation_snapshot_id, source_snapshot_id)
);

ALTER TABLE hosted_source_status ADD COLUMN publication_approval_id uuid REFERENCES evaluation_publication_approvals(id);
UPDATE hosted_source_status SET publication_status = 'hidden', publication_approval_id = NULL;
ALTER TABLE hosted_source_status ADD CONSTRAINT hosted_source_status_publication_binding_check
    CHECK ((publication_status = 'public') = (publication_approval_id IS NOT NULL));

CREATE INDEX evaluation_publication_approvals_evaluation_idx ON evaluation_publication_approvals (evaluation_snapshot_id);
