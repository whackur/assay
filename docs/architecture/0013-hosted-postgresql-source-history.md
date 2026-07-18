# 0013 - Hosted PostgreSQL source history and job boundary

## Status

Accepted.

## Context

Assay needs a first hosted path from a public GitHub submission to durable
source facts, a provider-transport attempt, and live source-processing status.
Container upgrades must not erase history, and missing provider output must
never be published as a zero score.

## Decision

PostgreSQL 17 is the authoritative hosted store. Local development uses a
Compose-managed volume. Production `compose.yaml` runs PostgreSQL, API, worker,
and web services, and mounts PostgreSQL and web state through explicitly named
external volumes. Only an explicit first-install command may create them;
routine deploy fails closed if a configured volume is absent. The helper
validates the fixed PostgreSQL 17 image and on-volume major before replacement,
backs up before deployment, and restores only into a new volume. Compose
therefore cannot delete production data with `down -v`.

The persistence boundary separates six kinds of record:

1. `source_requests` preserve normalized user intent and are idempotent by
   provider and requested owner/name.
2. `github_repositories` reconcile that request to GitHub's stable numeric
   repository identity, allowing owner/name changes without losing history.
3. `github_observations` append normalized public facts with ETag, source URL,
   collection time, and a content hash. Full source blobs, raw diffs, raw API
   payloads, and token-bearing values are forbidden.
4. `source_snapshots` bind revision plus the exact metadata observation. A
   same-revision metadata change therefore creates a new immutable snapshot;
   provider attempts bind to precisely the snapshot they received.
5. `analysis_jobs` use generation-fenced leases for at-least-once workers.
   Every mutation proves the current job generation, lease generation, token,
   and unexpired lease. `job_stage_attempts` preserve each bounded retry.
6. `analysis_capacity_reservations` serialize global provider capacity.
   Independent one-way anonymous-client, repository-owner, and provider buckets
   enforce burst cooldowns and recent-failure circuits. Raw client addresses do
   not cross the web boundary. Seed admission is explicitly internal but remains
   capacity bounded.

`crates/assay-github` owns GitHub's fixed API origin, no-redirect HTTP client,
response DTOs and byte bounds, ETag and rate-limit capture, canonical revision
resolution, error taxonomy, and the workflow collection-port adapter. It does
not own application sequencing or persistence.

`crates/assay-ai-evaluator` owns the Ollama/OpenAI-compatible provider profile.
It validates the operator-configured `/v1` base, calls `/chat/completions`
non-streaming, bounds the response while reading it, uses textual JSON message
content, records the stable provider ID
`ollama-openai-compatible-api-1`, and routes all untrusted output through the
canonical deny-unknown judgment and evidence-citation validator. Configuration
and credentials remain environment-only; repository submissions cannot choose
endpoints. A validated qualitative judgment is still not published or stored
as provider prose in this slice. The deterministic score compiler is not wired,
so a canonically validated attempt is persisted as `validated_unpublished`:
complete for transport and validation, unavailable for score and publication.
Only bounded provenance, usage, latency, and exact source identities are
stored; provider prose and ratings are not persisted by this workflow.

`crates/assay-project-intelligence` owns the provider-independent hosted
workflow, retry-policy inputs, typed source-status machine contract, and
projection policy. Storage, GitHub, and Ollama implement its ports. The API
serializes its DTOs directly. The worker only parses deployment configuration,
wires adapters, and drives the shared workflow, so sequencing is not duplicated
in an entrypoint.

The current job stage and exact source snapshot are durable. An evaluation-only
retry resumes from preserved normalized facts instead of recollecting GitHub.
GitHub and Ollama retry/reset hints are retained and scheduling honors the
larger provider delay, subject to an operator-configured maximum.

Migrations are forward-only and serialized with a PostgreSQL advisory lock.
Production fixes the Compose image to `postgres:17-alpine` and rejects operator
overrides, so an unknown or mismatched major cannot touch the active volume.
Minor upgrades use verified logical backup
plus deploy; major upgrades require a fresh target volume and a rehearsed
logical restore or `pg_upgrade` procedure. Older application images may be
rolled back only when compatible with the forward schema.
Seed repositories use bounded capacity but never force-refresh an existing
terminal job. Public resubmission explicitly creates a new job generation after
the applicable cooldown or failure backoff.

The deployment-internal HTTP routes remain `/internal/hosted/*`, but their
payload is the exact `assay-hosted-api` `1.0.0` contract defined by
`schemas/hosted-api/1.0.0.json`. Rust response DTOs are independent of storage
rows and are validated against that schema. Web types are generated
deterministically from the same file and bind its SHA-256 source hash. The
route location is not advertised as a general public API; versioning describes
the payload actually validated rather than claiming broader availability.
The read model is recent source-processing status, not a product catalog or
publication approval. Its backing rows default to hidden until a future
publication policy exists.

## Alternatives

- **Keep production web-only.** Rejected because it would advertise a hosted
  experience without deploying the durable source workflow that backs it.
- **Key repositories by owner/name.** Rejected because GitHub renames and
  transfers would split one repository's history.
- **Persist raw GitHub responses.** Rejected by Assay's privacy and retention
  boundary. Normalized, provenance-bearing facts are sufficient for this slice.
- **Publish a provisional numeric score from metadata.** Rejected because the
  product requires a versioned rubric, validated evidence citations, and a
  deterministic compiler.

## Consequences

- Recreating any service preserves database history because production uses a
  stable external volume. Operators must keep verified backups off that volume
  and rehearse restore into a fresh volume.
- Collection and provider failures remain visible as partial or unavailable
  states rather than becoming zeros. Temporary failures retry with bounded
  exponential/provider-requested backoff; abandoned or exhausted leases
  reconcile terminally. Project pages poll the same-origin status route with a
  fixed attempt bound and stop at a terminal state.
- This slice can grow by adding observation kinds and compiler-produced
  snapshots without rewriting prior facts.
- Hosted contract changes within v1 must remain backward compatible. A field
  removal, required-field addition, or semantic change requires a new major
  schema artifact and coordinated Rust/web regeneration.
- Production deployment fails closed if backup validation, migration health,
  internal API health, worker liveness, or loopback web verification fails.
