# Assay Agent Instructions

## Authority and scope

These rules apply to the entire repository. A more specific `AGENTS.md` in a
subtree may inherit and refine them.

Product specifications under `docs/specs/` are the implementation source of
truth. Implementation handoffs under `docs/internal/handoffs/` are point-in-time
context and never override specifications.

## Product boundaries

Assay has two separate domains:

1. **Contribution intelligence** reports explainable signals about accepted
   changes and collaboration. It must never produce a person-level score,
   leaderboard, productivity claim, or compensation signal.
2. **Project intelligence** evaluates public open-source projects. Project
   scores may be dimensioned, versioned, confidence-aware, and evidence-backed,
   but must not be reused to evaluate individuals.

Always preserve raw observations separately from derived metrics. Represent
uncertainty and missing data explicitly; never convert unavailable evidence to a
misleading zero. Treat churn as a rework or risk signal, not a productivity
penalty.

## Architecture ownership

| Area | Owner |
| --- | --- |
| Core domain types | `crates/assay-domain` — no database, HTTP, GitHub, or UI dependencies |
| Git history | `crates/assay-git` |
| GitHub collection | `crates/assay-github` |
| File classification | `crates/assay-classifier` |
| Structural diff | `crates/assay-semantic-diff` behind replaceable interfaces |
| Metric calculation | `crates/assay-metrics` using domain inputs only |
| Persistence adapters | `crates/assay-storage` |
| Identity, OIDC, sessions, entitlements, API-token policy | `crates/assay-identity` using validated `(issuer, subject)` identities |
| Project profiles, scores, similarity, and catalog contracts | `crates/assay-project-intelligence` |
| AI rubrics, evidence validation, and provider adapters | `crates/assay-ai-evaluator` |
| Public Codex authorization and isolated per-user execution | `apps/assay-codex-broker` |

Keep `crates/assay-cli`, `apps/assay-api`, and `apps/assay-worker` thin. Keep
business rules out of `web/`; it renders Rust API contracts. Keep `skills/assay`
thin and dependent on the public CLI's versioned JSON output.

## Development rules

- Start analysis-behavior changes with a minimal synthetic or redistributable
  fixture.
- Use TDD for classifiers, identity rules, change-set formation, semantic diff,
  churn windows, and metric formulas.
- Add golden JSON tests for CLI and Agent Skill contracts.
- Record an ADR under `docs/architecture/` for major boundary, persistence,
  semantic-diff engine, or public-schema changes.
- Keep changes small, reversible, and focused. Do not mix metric-policy changes
  with unrelated refactors.
- Keep dependencies minimal and justify new runtime services.

## Required verification

Run the relevant subset and report exactly what ran:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

For `web/` changes, also run the package-defined lint, type-check, tests, and
production build. For CLI output changes, validate the JSON schema and update
goldens intentionally. Report unavailable tooling and unrelated pre-existing
failures; never claim unexecuted verification.

## Source file size

For hand-written implementation source files, 200 nonblank/noncomment LOC is a
decomposition smell: plan a split. Above 300 nonblank/noncomment LOC, splitting
into focused modules is required before merge. Only generated files, vendored
files, and externally constrained declarative artifacts are exempt; exceptions
require a documented rationale in the PR or change description.

## Language and documentation

- Source code, identifiers, comments, commit messages, public documentation,
  schemas, fixtures, issues, and pull requests are English.
- Prose under `docs/internal/` is Korean by default. Preserve commands,
  identifiers, machine values, and quoted public contracts in English.
- User-facing product output is English by default.
- Metric definitions must state what they measure, what they cannot measure,
  and common misinterpretations.

## Data and security

- Default to read-only Git and GitHub collection.
- Never commit or log tokens, secrets, private source, credential-bearing
  payloads, provider-specific private claims, or private infrastructure details.
- Keep provider credentials in server-side secret storage. Public
  repository-submission forms must never accept Codex `auth.json`, ChatGPT OAuth
  tokens, or OpenAI API keys.
- Do not store full source blobs or raw diffs in PostgreSQL by default. Store
  hashes, locations, extracted facts, and provenance; retain source only in an
  explicitly configured cache.
- Telemetry is opt-in, and local CLI analysis must work without it.
- Automatic identity matches are reviewable suggestions. Destructive merges
  require explicit, auditable confirmation.

## Public contracts

- Send machine results to stdout and diagnostics to stderr.
- Support non-interactive execution, stable exit codes, `--no-color`, and
  versioned JSON schemas.
- Include analysis version, rule-set hash, source revision, provenance, warnings,
  and data sufficiency.
- Preserve compatibility within a schema major version.
- Agent Skills use scoped, revocable Assay API tokens, never browser cookies or
  upstream OIDC refresh tokens.
- Project-intelligence and AI judgments must cite evidence. A deterministic,
  versioned compiler publishes scores from validated judgments.
- Never invent Codex OAuth endpoints or request pasted token material.
  Experimental OAuth remains feature-flagged, isolated, encrypted, revocable,
  auditable, and kill-switch controlled.

## Generated and local files

- Commit appropriate lockfiles.
- Never hand-edit generated code or schemas; update the source or generator and
  regenerate.
- Never commit build output, caches, credentials, private repository data, or
  benchmark clones.
