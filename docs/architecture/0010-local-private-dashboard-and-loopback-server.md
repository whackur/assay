# 0010 — Local private dashboard, loopback server, and history store

## Status

Accepted.

## Context

The first public MVP includes local private-repository analysis with a browser
dashboard (specifications 5.10, 12.6, and requirement WEB-004). This introduces
concerns absent from the deterministic CLI slice:

- a dashboard served over HTTP that must bind only the loopback interface;
- a named-environment-variable GitHub PAT whose value must never appear in an
  argument, log, result, error, stored record, or remote request;
- immutable local history that accumulates across rescans and is mutated only by
  the local operator; and
- consent gating for private-source AI evaluation and public-competitor
  discovery, disabled by default.

`assay-cli` is a thin entrypoint and `assay-project-intelligence` must stay free
of I/O, so none of these belong there. We also want no new long-running runtime
service or heavyweight HTTP dependency for a single-user local tool.

## Decision

Introduce a `crates/assay-local` library that owns the local single-operator
surface, and keep `assay-cli` a thin wire-up over it.

- **Loopback binding.** `LoopbackListener` wraps `std::net::TcpListener` and
  exposes only a `bind(port)` constructor that always uses `Ipv4Addr::LOCALHOST`.
  No constructor accepts a caller-chosen address, so binding a routable
  interface is unrepresentable rather than validated away. `serve::run` accepts
  only a `LoopbackListener`.
- **Minimal HTTP.** The dashboard speaks a tiny read-only HTTP/1.1 subset over
  the standard library. Request routing operates on a `BufRead` line and
  responses are plain bytes, so there is no third-party HTTP dependency. It
  serves `/api/health`, `/api/history`, and `/api/history/{id}` returning the
  versioned local report contract; the web frontend can point its thin client at
  this API without change to shared components.
- **Token boundary.** `GithubTokenEnvVar` holds only a variable *name*.
  `SecretToken` redacts its `Debug`, implements neither `Display` nor
  serialization, and exposes bytes only through `expose_for_authorization` at the
  transport seam. `PrivateFetchRequest` structurally omits any token field, so
  serializing or logging a request cannot leak credentials. Real remote fetch is
  out of scope; the transport trait is the seam and default builds carry no
  implementation.
- **History store.** `LocalHistoryStore` is a file-based, append-only store:
  each analysis writes an immutable `records/NNNNNN.json` created with
  `create_new`, so a rescan never overwrites a prior snapshot. Soft deletion,
  restoration, and purge are append-only journal operations that require a
  `LocalAdministrator` capability, representing the single local operator. There
  is no database.
- **Consent and sections.** `ConsentState` defaults to no grants, rendering each
  private feature `disabled` with reason `user_consent_required` and next action
  `grant_consent`. A `ConsentGrant` cannot exist without naming a provider and
  the transmitted-evidence scope. `LocalReport` is always `private_local` and
  never catalog-eligible, keeping private source and its derivatives out of the
  public catalog and comparison corpus.

## Alternatives

- **Axum/hyper server.** Rejected: a routable, dependency-heavy async server is
  disproportionate for a loopback single-user tool and widens the attack surface.
- **Embed serve/history in `assay-cli`.** Rejected: the CLI must stay a thin
  entrypoint, and the logic needs unit and contract tests independent of process
  spawning.
- **SQLite/Postgres history.** Rejected: the deferred-work list excludes local
  persistence engines here; immutable JSON files satisfy accumulation, operator
  control, and determinism without a database.
- **Runtime string scrubbing of tokens.** Rejected in favor of a type that
  cannot be printed or serialized, so non-exposure is structural rather than a
  filter that can be forgotten.

## Consequences

- The loopback-only guarantee is enforced by construction and covered by a real
  round-trip test.
- Token non-exposure is proven by a mutation-style test that plants a token and
  asserts its absence across the debug output, transport request, resolution
  error, report JSON, and on-disk record bytes.
- History accumulates immutably; only the local administrator can soft-delete,
  restore, or purge.
- The HTTP subset is intentionally minimal; broadening routes or methods later
  is additive within the versioned contract.
