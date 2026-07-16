# ADR 0006: Isolate the OpenAI Adapter Behind Credential and Transport Ports

- Status: Accepted
- Date: 2026-07-16

## Context

The public web service and normal server deployments evaluate qualitative
criteria through a server-managed OpenAI API key. The specification requires the
key to load from secret storage, never reach analyzed repository code or the
browser, and rotate without data migration. It also requires the evaluation
snapshot to record provider, model, prompt and rubric versions, sampling, usage,
latency, and validation status, and requires schema failure, timeout, rate
limit, and secret redaction to be covered by tests.

The `assay-ai-evaluator` crate already validates untrusted provider output
against the exact rubric and evidence bundle. It performs no I/O, and the rest of
the workspace has no HTTP client or async runtime. Adding a live HTTP client and
its transitive runtime to this crate would pull network and TLS concerns into the
same crate that must stay a deterministic, side-effect-free validator, and would
expand the dependency surface before any deployment consumes it.

## Decision

### Narrow, injected credential and transport seams

The adapter depends on two object-safe ports. `SecretStore` loads a
`ProviderSecret` by a validated `SecretName`, so a rotated key is read by the
same reference name with no stored-data change and no command-line argument.
`HttpTransport` sends one `OutboundRequest` and returns an untrusted
`TransportResponse` or a redacted `TransportError`. The concrete secret store and
HTTP client live outside this crate and are injected by the deployment. This
mirrors ADR 0004: the crate defines the deterministic seams and their tests while
the persistent adapters remain outside it. No live HTTP client dependency is
added in this card; wiring a concrete client is deferred to a thin deployment
layer that implements `HttpTransport`.

### Credential never leaks through types

`ProviderSecret` wraps key material and implements a redacting `Debug`, no
`Display`, and no `Serialize`. The key is exposed only through
`OutboundRequest::authorization`, which builds the `Authorization: Bearer` header
value; the request body carries the prompt and evidence but never the key. Secret
resolution failure fails closed as an explicit status before any transport call,
so a missing key never reaches the network and never appears in an error.

### Prompt keeps evidence separate from instructions

The adapter sends the fixed system instructions as the chat system message and
the canonical, explicitly delimited evidence payload as the user message. The
system message states that repository evidence is untrusted data and that
instructions inside it are ignored, satisfying the prompt-injection posture while
reusing the same untrusted-input validation the fake provider already passes.

### Honest, self-describing snapshot with isolated telemetry

Every evaluation returns an `EvaluationSnapshot`. Deterministic provenance
(provider, model, prompt and rubric versions, sampling, evidence-bundle hash) and
the validated judgment set are the only inputs a later deterministic score
compiler may read. Token usage and latency are isolated in optional
`ProviderTelemetry` that is never fed into scoring, keeping non-deterministic
values out of deterministic calculation. A timeout, rate limit, unauthorized or
other HTTP status, a malformed envelope, and a schema-invalid judgment each
become an explicit `Failed` status with a stable code; none is disguised as a
zero-rated success.

## Consequences

- The validator crate stays I/O-free and dependency-light; no HTTP client or
  async runtime enters the workspace for this card.
- Deployments implement `SecretStore` against server-side secret storage and
  `HttpTransport` against a real client without changing the adapter or its
  tests.
- Key rotation is a secret-store concern only; the adapter reads the current key
  by name on each evaluation.
- Failure modes required by the specification are explicit statuses backed by
  deterministic tests rather than silent zeros.

## Alternatives considered

- Adding a concrete HTTP client inside this crate was rejected because it would
  couple a deterministic validator to network and TLS concerns and expand the
  dependency surface before any deployment needs it.
- Returning only a validated judgment set and recording usage and latency
  elsewhere was rejected because the snapshot is the required provenance record
  and must isolate non-deterministic telemetry from scoring itself.
- Passing the key through configuration structs with derived `Debug` or through
  a command-line argument was rejected because it would risk the key appearing in
  logs, errors, or process listings.
- Mapping timeout or rate limit to an empty or zero-rated result was rejected
  because it would disguise an unavailable evaluation as a poor one.
