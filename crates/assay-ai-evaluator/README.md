# assay-ai-evaluator

This crate owns Assay's provider-independent qualitative project rubric,
bounded evidence bundle, provider adapter port, the server-managed OpenAI API
adapter, and validation of untrusted AI judgments. It defines the credential
and HTTP transport ports the adapter needs but performs no filesystem, process,
network, database, credential, or score-compilation I/O itself; the concrete
secret store and HTTP client are injected from the deployment.

## What it measures

The initial `project-rubric-1` asks a provider for bounded 0–4 judgments about:

- correspondence between project claims and cited implementation evidence;
- differentiation supported by cited project or comparison evidence;
- coherence of documented project scope; and
- credibility of a future-potential narrative.

Each result records the evaluation and rubric versions, applicability,
confidence, exact evidence-bundle hash, citations, and bounded rationale.
These are qualitative project judgments, not published scores.

## What it cannot measure

The evaluator does not establish that a repository builds, runs, is secure,
original, useful, or likely to succeed. It does not execute repository code,
contact a model provider, calculate an Assay Score or Potential, or evaluate a
person. A future deterministic score compiler may consume only the validated
rating, applicability, confidence, and evidence IDs; provider rationale remains
explanation text and is excluded from that scoring view.

## Common misinterpretations

- A rating is a bounded rubric judgment, not a direct score contribution.
- Provider confidence is not statistical certainty or project-score
  confidence.
- A citation proves that an evidence ID was in the supplied bundle; it does not
  prove the provider interpreted that evidence correctly.
- `not_applicable`, unavailable, insufficient, and pending states are not zero
  ratings.
- The contract evaluates project evidence and must never be used to infer a
  contributor's productivity, intent, compensation, or performance.

## Trust and privacy boundary

Evidence descriptors are length-bounded and reject prompt-injection markers,
credential-bearing content, raw diffs, and absolute host paths. Private-local
evidence cannot use the public-only transmission mode. The canonical prompt
keeps fixed instructions separate from an explicitly delimited JSON evidence
payload.

Provider bytes are untrusted. Validation rejects malformed or schema-invalid
output, version or bundle mismatches, unknown or duplicate criteria,
out-of-range ratings and confidence, missing or duplicate citations, citations
outside the supplied bundle, unsafe provider prose, and person-domain mixing.
Errors and debug output retain no provider response or evidence statement.

The authoritative public contract is
`schemas/ai-judgment/v1.json`; Rust serialization is validated against that
reviewed schema in the contract tests.

## OpenAI API adapter

`OpenAiEvaluator` implements the server-managed OpenAI API provider mode. It
loads the key from a `SecretStore` by reference name, so a rotated key is read
without data migration and is never passed as a command-line argument. The key
is wrapped in a `ProviderSecret` that never appears in `Debug`, `Display`,
errors, logs, or serialization; it leaves the process only as the `Authorization`
header value of one `OutboundRequest`, never in the request body. The concrete
HTTP client and secret store live outside this crate and are injected through
the `HttpTransport` and `SecretStore` ports; the adapter logic is proven end to
end with a deterministic in-memory transport, so tests make no network calls.

Every evaluation returns an `EvaluationSnapshot` that always records
deterministic provenance (provider, model, prompt and rubric versions, sampling
settings, evidence-bundle hash) and an explicit outcome. Token usage and latency
are isolated in optional non-deterministic telemetry that never feeds score
compilation. A timeout, rate limit, unauthorized or other HTTP failure, a
malformed envelope, and a schema-invalid judgment each become an explicit failed
status; none is disguised as a zero-rated success.
