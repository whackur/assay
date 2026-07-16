# assay-ai-evaluator

This crate owns Assay's provider-independent qualitative project rubric,
bounded evidence bundle, provider adapter port, and validation of untrusted AI
judgments. It performs no filesystem, process, network, database, credential,
or score-compilation work.

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
