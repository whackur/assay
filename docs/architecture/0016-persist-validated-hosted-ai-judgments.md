# ADR 0016: Persist validated hosted AI judgments

- Status: Accepted
- Date: 2026-07-19

## Context

The hosted evaluator validates the canonical `ai-judgment/v1` artifact, but
the initial hosted persistence slice discarded it. Successful evaluation
snapshots therefore recorded only provenance and telemetry as
`validated_unpublished`, even though no validated judgment was available for a
future deterministic score compiler.

## Decision

Persist the validated `ai-judgment/v1` JSON artifact with a successful hosted
evaluation snapshot. The artifact remains an unpublished evaluator input:

- `evaluation_snapshots.status` remains `validated_unpublished`.
- `score_status` remains `unavailable` until the deterministic compiler creates
  a separately versioned score artifact.
- The public hosted API does not expose the judgment, rationale, or ratings.

A forward-only migration permits non-null judgments only for validated
snapshots and binds their schema version, evaluation version, rubric version,
and evidence-bundle hash to the snapshot provenance. Existing null judgments
remain valid historical records.

## Consequences

- Hosted evaluation history retains the validated, evidence-bound input needed
  for later deterministic compilation without treating it as a public score.
- Provider rationale remains outside public API contracts and score compilation
  continues to use the bounded scoring view that excludes rationale.
- Legacy validated snapshots without a judgment remain unavailable rather than
  being retroactively inferred or rewritten.
- Older binaries remain compatible with the expanded nullable database column;
  application tests require newly validated attempts to provide a judgment.
