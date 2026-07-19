# ADR 0014: Wire the AI Evaluator and Score Compiler into the CLI

- Status: Accepted
- Date: 2026-07-19

## Context

ADR 0007 placed the deterministic score compiler in `assay-project-intelligence`
and the shared judgment contract in `assay-domain`, and ADR 0012 defined the
pluggable `EvaluationProvider` port. The foundation milestone proved the chain
end to end through integration tests, but the handoff
(`docs/internal/handoffs/2026-07-16-mvp-integration-status.md` §2.3) recorded
two deliberately deferred wiring steps:

1. **ADP-001**: no adapter converted the CLI's `ProjectEvidenceManifest` into
   the evaluator's `EvidenceBundle`, so integration tests assembled the bundle
   by hand from CLI-emitted evidence identifiers.
2. **WIRE-001**: `assay project analyze` produced evidence only; the AI
   evaluator and score compiler were not wired into the CLI, and
   `assay capabilities` reported `ai_evaluation` and `project_scores` as
   `not_implemented`.

## Decision

### ADP-001: manifest-to-bundle adapter in `assay-ai-evaluator`

A new `adapter` module in `assay-ai-evaluator` converts a
`ProjectEvidenceManifest` into a provider-safe `EvidenceBundle` without raw
source, diffs, host paths, or person-level language. Every raw and
classification fact becomes one bounded `EvidenceDescriptor` whose statement is
derived from the fact's kind and availability, never from the underlying
content. The adapter is deterministic and performs no I/O; identical manifests
yield byte-identical bundles, so the downstream bundle hash is stable.

The adapter carries an explicit `AdapterPrivacy` (scope and transmission
policy). The default `local_deterministic` keeps evidence `PrivateLocal` with
`NotUsed` external transmission, matching ADR 0003's local-input contract.

### WIRE-001: CLI wiring through the deterministic evaluator and compiler

`assay project analyze` now runs the adapter, the `DeterministicFakeProvider`,
and the score compiler after assembling the manifest. The chain is
deterministic and network-free by default. The compiled `project-evaluation`
instance is embedded as an optional `evaluation` field in the `project-analysis`
bundle, so the CLI still emits one JSON result while exposing the evaluation
contract.

### Schema extension: optional `evaluation` in `project-analysis/v1`

The `project-analysis` schema gains an optional `evaluation` field referencing
`project-evaluation/v1.json`. The field is absent when no evaluator runs and
present when the deterministic evaluator and score compiler produce an
evaluation. This is an additive, backward-compatible change: existing
`project-analysis` instances without the field remain valid.

### Capability reporting

`assay capabilities` now reports `ai_evaluation` and `project_scores` as
`implemented`. The `ai_evaluation` feature lists every registered evaluator ID
with its family and per-binary status; the deterministic evaluator claims
`implemented` because the CLI wires it end to end through to a validated
judgment set. The external AI evaluator IDs (`openai-api-1`, `codex-cli-1`)
remain `not_implemented` until a live provider is constructed.

### Consent gating

Private-source AI processing requires explicit consent (ADR 0012). The local
slice exposes no consent-granting surface yet, so the recorded report keeps its
`ai_evaluation` section `disabled` with `user_consent_required`. The
deterministic evaluator runs without external transmission, so it runs without
consent; external providers are not constructed.

### Public numeric Assay Score gating

The public numeric Assay Score remains behind the sufficiency and calibration
gates in the compiler. When essential dimensions cannot be scored, the score is
`insufficient` with a null value, never a zero. The `score_release_gate_not_met`
warning is emitted while the gate is not met.

## Consequences

- The CLI now emits a `project-evaluation` instance embedded in the
  `project-analysis` bundle, validated against the public schema.
- The deterministic evaluator and score compiler are wired end to end; the
  `ai_evaluation` and `project_scores` capabilities flip to `implemented`.
- The manifest-to-bundle adapter is the single boundary between
  `assay-project-intelligence` and `assay-ai-evaluator`, so future provider
  families reuse the same bundle construction.
- The `project-analysis` schema gains an optional field; existing consumers
  remain valid.
- External AI providers remain consent-gated and unimplemented; the local
  slice still constructs no external provider.
- The public numeric Assay Score stays gated; the field is present but null
  while essential dimensions remain unscored.

## Alternatives considered

- **Emitting `project-evaluation` as a separate CLI output** was rejected
  because the CLI must return one JSON result and the evaluation is derived
  from the same evidence bundle.
- **Adding the evaluation as a non-optional field** was rejected because it
  would break existing consumers that do not run the evaluator.
- **Placing the adapter in `assay-cli`** was rejected because the adapter
  bridges two library crates and belongs with the evaluator boundary.