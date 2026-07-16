# 0011 — Run stage state, bounded retries, and administrator recovery

## Status

Accepted.

## Context

The analysis pipeline runs the named stages of the product interview
(specification 14 job stages and requirements OPI-012, PWS-009): source
verification, revision pinning, file and history analysis, project-type
determination, CI and dependency evidence, similar-project discovery, AI rubric
evaluation, score compilation, and result publication.

The product requires that a partial stage failure never fails the whole run.
Completed stages and their immutable snapshots must be preserved while only
failed stages are marked `partial` or `unavailable` with a reason. The system
retries a failed stage a bounded number of times; once that budget is spent,
ordinary users have no retry path. Only an administrator may rerun failed
stages, soft delete, restore, or purge a run, and every such action must be
audited without secret material.

The real worker, queue, and PostgreSQL model are out of scope here. The
deliverable is the state machine, the versioned retry policy, the administrator
gate, and the audited recovery contract, with their tests and an additive
public schema.

Two existing patterns are relevant. `assay-local` owns an append-only,
operator-gated journal for soft delete, restore, and purge of immutable local
records. `assay-identity` owns local roles, the `analysis.admin.rerun` and
`analysis.admin.delete` entitlements, and a secret-free `AuditEvent`.

## Decision

Add a `run` module to `crates/assay-project-intelligence` that owns the run
stage state machine and administrator recovery operations.

- **Stage model.** `Stage` enumerates the nine named stages in canonical order.
  `ProjectRun` holds one `StageState` per stage. A stage status is one of
  `pending`, `complete`, `partial`, or `unavailable`. Completed and partial
  stages retain a `ContentHash` result snapshot; failed stages carry a redacted
  snake_case reason. The four-state vocabulary deliberately mirrors the domain
  availability states without importing them, because a stage status is a
  pipeline position, not an evidence-availability fact.
- **Preserving partial failure.** `record_attempt` settles one bounded worker
  attempt and never touches other stages, so a failure preserves every
  completed stage. `ProjectRun::status` derives `partial` for any mixed run and
  never reports `complete` unless all stages are complete; `unavailable` and
  `partial` are therefore never disguised as a success or a zero.
- **Bounded retries as policy data.** `RetryPolicy` carries a versioned
  `automatic_retry_budget`, not a scattered constant. A failed attempt with
  budget remaining keeps the stage `pending` (`RetryScheduled`); the attempt
  that exhausts the budget makes the stage terminally `unavailable`
  (`Exhausted`). A terminal stage rejects further recording, so there is no
  automatic or ordinary-user retry path. `ordinary_user_retry_available` is a
  constant `false`.
- **Capability-gated recovery.** `rerun_stage`, `rerun_failed_stages`,
  `soft_delete`, `restore`, and `purge` each require an `Administrator`
  capability token, mirroring the `LocalAdministrator` pattern of `assay-local`.
  The token represents authorization already established by the identity layer's
  `analysis.admin.*` entitlements; this crate imports no role source. A stage
  rerun is permitted only on a failed stage and reuses every completed stage's
  immutable snapshot. `RunLifecycle` mirrors the local journal's active,
  deleted, and purged states; purge drops result content but retains the audit
  trail.
- **Audit without secrets.** Every recovery action appends an `AdminAuditEvent`
  carrying the action, run id, optional targeted stage, policy version, and an
  injected timestamp. It follows the secret-free principle of the identity
  `AuditEvent` but stays in this crate to respect the inward dependency
  direction.
- **Determinism.** The module performs no clock, filesystem, process, or network
  I/O. Timestamps and identifiers are injected, so identical input yields
  byte-identical machine output.
- **Public contract.** `schemas/run-state/v1.json` is an additive public schema
  with a reviewed golden and an invalid fixture. It binds reason and snapshot
  presence and `automatic_retries_exhausted` to each stage status, forbids an
  ordinary-user retry, and requires a single-stage rerun to name its stage while
  the other actions do not.

## Alternatives

- **Put the state model in `assay-domain`.** Rejected: the nine named stages are
  a project-intelligence pipeline structure, not a provider-agnostic domain
  value. The domain crate contributes only the reused availability vocabulary
  and `ContentHash`.
- **Extend the `assay-identity` `AuditEvent` for run actions.** Rejected as the
  home for the run operations: `assay-project-intelligence` must not depend on
  an adapter crate, and the identity audit record has different, mapping-shaped
  fields. The identity entitlements still gate these actions at the application
  boundary; the ADR records the parallel.
- **Reuse `assay-local`'s journal directly.** Rejected here: that store is a
  loopback single-operator, file-based surface without stage-level partial
  failure or a rerun-stage operation. The in-memory state machine is the unit
  and contract deliverable; persistence stays consistent with, but separate
  from, that journal.
- **Allow a user-triggered retry after exhaustion.** Rejected by the
  specification: recovery is administrator-only and audited.

## Consequences

- Partial failure, bounded retries, exhaustion, and administrator-only recovery
  are enforced by construction and covered by unit and schema-contract tests.
- The audit trail survives purge, so a destructive action remains reviewable
  even after its result content is dropped.
- Wiring a real worker, queue, and persistence layer is additive: they drive
  `record_attempt` and the recovery operations and persist the versioned
  machine value without changing the contract.
