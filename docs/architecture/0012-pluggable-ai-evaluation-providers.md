# ADR 0012: Pluggable AI-Evaluation Providers Behind the EvaluationProvider Port

- Status: Accepted
- Date: 2026-07-17

## Context

`assay-ai-evaluator` already defines the provider boundary: the
`EvaluationProvider` trait returns untrusted bytes from a canonical
`ProviderRequest`, and `Evaluator` validates those bytes against the exact
rubric and `EvidenceBundle` (schema version, evaluation and rubric versions,
evidence-bundle hash binding, citation existence, rationale text policy, and a
bounded output size). ADR 0006 established the injected `SecretStore` and
`HttpTransport` ports for the OpenAI adapter, and the crate performs no
network, filesystem, process, or credential I/O itself. The CLI, however, still
reports the `ai_evaluation` capability as `not_implemented`, and no provider is
selectable from `assay project analyze`.

The product needs two provider families (specification section 9.6):

1. **API-key HTTP providers.** Any LLM API reachable with a server- or
   operator-managed key — OpenAI today, Anthropic and others next. These
   receive only the bounded evidence bundle, so evidence selection must fit a
   context window.
2. **Agentic CLI providers.** A coding-agent CLI (for example Codex CLI
   authenticated through its own official login, or Claude Code) runs inside a
   cloned repository snapshot and explores the working tree itself instead of
   receiving a full evidence payload. This sidesteps context-window limits on
   large repositories, at the cost of a subprocess boundary and a broader
   transmission surface.

The specification already constrains the local Codex mode to a read-only
sandbox, a fixed output schema, bounded time and resources, and no credentials
supplied by repository content. This record decides how both families plug
into the existing port without creating a second trust path.

## Decision

### One port, two adapter families, one validator

Both families implement the existing `EvaluationProvider` trait. Every
provider — HTTP or subprocess — returns untrusted bytes from `evaluate()`, and
those bytes pass through the existing `Evaluator` validation unchanged: schema
shape, evaluation and rubric version equality, `evidence_bundle_hash` binding,
citation existence in the exact bundle, rationale text policy, and the bounded
output size. No adapter family gets a shortcut into the score compiler, and no
new trust path is introduced. `enforce_transmission_boundary` continues to
gate every evaluation before any provider work starts.

### API-key HTTP family

The family generalizes the `OpenAiEvaluator` pattern. Shared across providers:

- the injected `SecretStore` and `HttpTransport` ports, `SecretName`
  resolution, and the redacting `ProviderSecret`;
- the `ProviderRequest` canonical payload with fixed system instructions
  separated from delimited evidence;
- the `EvaluationSnapshot` record with deterministic provenance and isolated
  non-deterministic telemetry; and
- the failure taxonomy (`provider_timeout`, `provider_rate_limited`,
  `provider_unauthorized`, `secret_unavailable`, and validation failures).

Per provider: the endpoint, model identifier, request-body shape (chat
envelope, message roles, response-format flags), authentication header form,
response-envelope extraction, and sampling-parameter names. Each concrete
adapter owns a stable `provider_id` and its own `Config` struct; a new API
provider is a new envelope builder and extractor over the same two ports, not
new validation logic.

### Agentic CLI family

A subprocess provider implements `EvaluationProvider` as follows:

1. **Materialize an immutable snapshot.** The host materializes the analyzed
   commit into a temporary working tree using the existing local Git snapshot
   machinery (the same resolved-commit discipline as ADR 0002). The snapshot
   is the exact tree of the analyzed revision, never the operator's live
   working copy.
2. **Write the task inputs.** Into a separate writable control directory the
   host writes the rubric, the canonical `ProviderRequest` payload, and the
   list of evidence items the agent MUST examine and cite. The instructions
   state that repository content is untrusted data and that only the listed
   evidence IDs are citable.
3. **Spawn the agent CLI** with the snapshot as its constrained working
   directory and the control directory as its only writable output location.
4. **Collect the judgment.** The agent writes one structured JSON judgment
   file to a designated output path. Those bytes are the untrusted return
   value of `evaluate()` and flow into the same `Evaluator` validation,
   including the existing output-size bound.

The crate stays I/O-free. Snapshot materialization and process execution are
injected ports, exactly as `HttpTransport` is: a `SnapshotWorkspace` port
materializes and disposes of the tree, and an `AgentRunner` port spawns one
bounded subprocess and returns untrusted bytes or a redacted error. Concrete
implementations live in the deployment layer, and deterministic test doubles
exercise the adapter.

**Sandbox requirements the host runner must enforce:**

- the snapshot is read-only to the agent; only the designated output path is
  writable;
- no network access except the agent's own model endpoint;
- no execution of repository code (the `repository_code_execution` capability
  remains `prohibited`); the agent reads files, it does not run builds, tests,
  hooks, or scripts from the tree;
- bounded wall-clock runtime, bounded output size, and bounded resource use,
  with a limit producing an explicit failure rather than a fabricated result;
  and
- the agent's credential store is used in place, through its official login;
  Assay never reads, copies, or transmits it, and repository content never
  supplies credentials.

**Prompt-injection posture.** Repository content is untrusted data and the
agent will read arbitrary amounts of it, so instruction-level defenses are
best effort only. The enforcement backstop is the existing validator: a
judgment citing evidence not in the bundle is rejected
(`unknown_evidence_citation`), version and hash mismatches are rejected, and
rationale text passes the same untrusted-text policy as every other provider.
An injected instruction can waste an agent run; it cannot smuggle an uncited
or unbounded judgment past validation.

**Execution boundary and consent.** This is where the current model needs
extension. `enforce_transmission_boundary` reasons only about the evidence
bundle: `External` transmission is legal when the bundle is `PublicOnly` or
when private evidence carries `ConsentedPrivate`. An agentic provider whose
model endpoint is remote is `ProviderExecutionBoundary::External` even though
the process runs locally, because worktree content reaches the model endpoint
— and it transmits more than the bundle: the agent may read and send any file
in the snapshot. The bundle-scoped consent in `ConsentGrant` (a provider plus
an acknowledged transmitted-evidence description) is therefore necessary but
not sufficient. The decision:

- The transmission model gains an explicit transmission *surface* alongside
  the existing scope: `bundle_only` (API-key family) versus
  `worktree_snapshot` (agentic family).
- A `ConsentGrant` for an agentic provider must acknowledge the
  `worktree_snapshot` surface by name — "this agent may read and transmit any
  file of the analyzed revision" — not merely the bundle facts. The existing
  free-text `evidence_scope` field is formalized to carry this surface.
- `enforce_transmission_boundary` (or its extended equivalent) rejects an
  agentic provider whose bundle consent does not include the snapshot
  surface, with the existing `privacy_mismatch` failure. A public-only
  repository still requires the surface acknowledgement, because the agent
  vendor receives the content even when it is public.
- An agentic provider with a fully local model endpoint may declare `Local`,
  which continues to require `ExternalTransmission::NotUsed`.

**Non-determinism.** Agentic judgments vary between runs. Providers never
aggregate: each `evaluate()` call is one run, and its validated result must
carry the `provider_id` plus run provenance (analyzed commit, agent CLI
identity and version, model identifier, and a run identifier) in its snapshot
record. Multi-run aggregation — for example median rating and citation
agreement across N validated runs — is deterministic post-processing on the
score-compiler side, outside the provider and outside this crate, so the same
set of validated runs always compiles to the same score.

## CLI wiring plan

`assay project analyze --evaluator <id>` selects a provider from a static
registry mapping stable evaluator IDs (for example `openai-api-1`,
`codex-cli-1`) to a family and its configuration. API-key providers resolve
credentials through a deployment `SecretStore` implementation reading
environment variables or the operating-system credential store by
`SecretName`; keys never appear in arguments or output. Agentic providers hold
no Assay-managed secret and require the agent's own authenticated
installation, probed before use the way ADR 0002 probes Git. Consent gating
runs before provider construction: without a matching `ConsentGrant` the
evaluation section stays `disabled` with `user_consent_required`. The
`capabilities` report flips `ai_evaluation` from `not_implemented` per
provider family as each lands, listing the implemented evaluator IDs so
automation can detect exactly which providers a binary supports.

## Consequences

- Both families reuse one validator, so provider count grows without growing
  the trust surface; a compromised or confused provider can only fail
  validation, not bypass it.
- The API-key family is bounded and closer to reproducible (recorded sampling,
  optional seed), but evidence selection must fit a context window, so
  coverage of large repositories depends on bundle curation quality.
- The agentic family covers large repositories the bundle cannot, but is
  slower, non-reproducible between runs, and has a larger attack surface: a
  subprocess, a sandbox the host must actually enforce, and a whole-snapshot
  transmission surface that demands broader consent.
- The consent model gains a transmission-surface dimension; dashboards and the
  report contract must present "bundle facts" and "full snapshot" consent as
  distinct acknowledgements.
- The workspace gains two injected ports (`SnapshotWorkspace`, `AgentRunner`)
  and their deployment implementations, plus host-level sandbox enforcement
  that must be tested with hostile repository content, oversized output,
  timeouts, and attempted network or write escapes.
- Score compilation gains a deterministic multi-run aggregation stage for
  agentic results; single-run agentic scores must be labeled as single-run.
- Implementation note: the agentic adapter landed as a snapshot-producing
  driver (`AgenticEvaluator`), mirroring the pre-existing OpenAI adapter
  shape, rather than as a literal `EvaluationProvider` impl — required run
  provenance (probed agent identity, run identifier) cannot travel through
  the trait's byte-only return. Both family drivers still route every
  untrusted byte through the one `Evaluator` validation path, and the trait
  itself gained the `transmission_surface` dimension so trait-based providers
  remain fully boundary-checked.

## Alternatives considered

- **A separate trait for agentic providers** was rejected: it would fork the
  validation path and invite subprocess output to reach the compiler through
  a second, less-tested route.
- **Sending the full worktree content through the HTTP family** was rejected:
  it reintroduces the context-window limit as a hard failure and moves file
  selection into prompt engineering instead of a deterministic bundle.
- **Letting the agent execute repository code for deeper evidence** was
  rejected: `repository_code_execution` is prohibited across Assay, and no
  judgment quality gain justifies running untrusted code.
- **Provider-side multi-run aggregation** was rejected: aggregation inside a
  non-deterministic provider is unauditable; keeping it in the deterministic
  compiler preserves the existing judgment contract boundary (ADR 0007).
- **Treating an agentic run as `Local` because the process is local** was
  rejected: the agent's model endpoint receives repository content, so the
  boundary is `External` whenever that endpoint is remote, and consent must
  say so.
