# Assay Project Instructions

## Scope

These instructions apply to the entire repository. Add a more specific
`AGENTS.md` only when a subtree needs rules that differ from this file.

The product sources of truth are the specifications under `docs/specs/`.
Research handoff files under `.orca/drops/` are inputs to the specifications,
not implementation contracts.

## Product Domains

Assay has two related but distinct product domains:

1. **Contribution intelligence** reports reference signals about accepted
   changes and collaboration. It never produces a person-level performance
   score.
2. **Project intelligence** evaluates public open-source projects for
   substance, originality, engineering rigor, open-source readiness,
   maintenance health, and potential. It may produce transparent
   project-level scores.

Do not mix person-level observations into a project score, and do not reuse a
project score to evaluate an individual contributor.

## Product Principles

- Build a reference-metrics tool, not a developer performance score.
- Measure reviewable evidence about activity, semantic impact, durability,
  collaboration, and risk reduction. Never claim to measure human value or
  productivity.
- Do not implement a composite person-level score, global contributor
  leaderboard, or default person-to-person ranking.
- Project-level scores must be dimensioned, versioned, confidence-aware, and
  fully explainable. An overall project score never replaces its dimensions.
- Evaluate the substance of the resulting project regardless of whether its
  authors used AI-assisted development tools.
- Prefer trends, uncertainty, provenance, and drill-down links to source
  changes.
- Treat churn as a rework or risk signal, not as a productivity penalty.
- Keep raw observations separate from derived and versioned metrics.
- Preserve contributions such as tests, documentation, CI, infrastructure,
  migrations, dependency updates, and security work as distinct categories.

## Language and Public Artifacts

- Write source code, identifiers, comments, commit messages, documentation,
  schemas, fixtures, issues, and pull request text in English.
- User-facing output must be English by default. Localization may be added
  later without changing machine-readable field names.
- Prefer plain, neutral language. Every metric definition must state what it
  measures, what it cannot measure, and common misinterpretations.

## Architecture Boundaries

- Keep `crates/assay-domain` free of database, HTTP, GitHub, and UI concerns.
- Put repository history extraction in `crates/assay-git` and GitHub-specific
  collection in `crates/assay-github`.
- Keep file policy in `crates/assay-classifier` and structural change analysis
  in `crates/assay-semantic-diff`.
- Implement metric calculation in `crates/assay-metrics` using domain inputs;
  it must not query GitHub or PostgreSQL directly.
- Put persistence adapters in `crates/assay-storage`.
- Put normalized external identities, OIDC contracts, sessions, entitlements,
  and Assay API-token policy in `crates/assay-identity`. Core code must not
  depend on a particular identity provider, website, Firebase project, or
  external application's user database.
- Make `crates/assay-cli`, `apps/assay-api`, and `apps/assay-worker` thin
  entrypoints over shared crates.
- Isolate public Codex authorization and per-user Codex execution in
  `apps/assay-codex-broker`. The API and general worker must never read raw
  Codex token material.
- Put public open-source profiling, project scoring, similarity evidence, and
  catalog-card contracts in `crates/assay-project-intelligence`. Keep this
  logic independent from person-level contribution metrics.
- Put versioned qualitative rubrics, evidence-citation validation, and AI
  provider adapters in `crates/assay-ai-evaluator`. The project-intelligence
  score compiler consumes validated judgments rather than provider prose.
- Keep business rules out of `web/`. The web application consumes the Rust
  API and renders reports.
- Keep the Agent Skill under `skills/assay` thin. It must invoke the public CLI
  and consume its versioned JSON output rather than reimplement analysis.
- Introduce interfaces around semantic-diff engines so tree-sitter,
  difftastic-derived logic, and GumTree can be evaluated or replaced.

## Repository Layout

```text
crates/       Rust libraries and the CLI package
apps/         Long-running Rust API and worker binaries
web/          Next.js and TypeScript dashboard
skills/       Agent Skill packages that wrap stable product interfaces
docs/         Product specifications, architecture decisions, and metrics
schemas/      Versioned machine-readable input and output schemas
configs/      Example configuration files only; never store credentials
migrations/   PostgreSQL migrations
scripts/      Deterministic development and release helpers
tests/        Cross-component fixtures, integration tests, and golden output
benchmarks/   Reproducible public-repository and performance benchmarks
```

## Development Method

- Gather or create a minimal fixture before changing analysis behavior.
- Use test-driven development for classifiers, identity rules, change-set
  formation, semantic-diff behavior, churn windows, and metric formulas.
- Make fixtures synthetic or derived from redistributable public sources.
- Add golden JSON tests for CLI and Agent Skill contracts.
- Record an architecture decision under `docs/architecture/` when changing a
  major boundary, persistence model, semantic-diff engine, or public schema.
- Keep changes small and reversible. Do not mix metric-policy changes with
  unrelated refactors.

## Required Verification

Run the relevant subset as the repository becomes executable:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

For changes under `web/`, also run the package-defined lint, type-check, test,
and production build commands. For CLI output changes, validate the JSON
schema and update golden files intentionally.

Do not claim verification that was not run. Report unavailable tooling and
unrelated pre-existing failures.

## Data, Privacy, and Security

- Default to read-only collection from Git and GitHub.
- Never log access tokens, private source contents, webhook secrets, or raw
  credential-bearing payloads.
- Keep OpenAI API keys in server-side secret storage. Never accept Codex
  `auth.json`, ChatGPT OAuth tokens, or OpenAI API keys through the public web
  form used to submit repositories.
- Configure human identity providers through standard OIDC metadata and
  deployment secrets. Do not commit provider-specific domains, client secrets,
  private claim conventions, or upstream application internals.
- Key external human identities by the validated `(issuer, subject)` pair, not
  by email. Map external claims to local entitlements through explicit
  deployment policy.
- Do not store full source blobs or raw diffs in PostgreSQL by default. Store
  hashes, locations, extracted facts, and provenance; use a configurable local
  or object-store cache when source retention is explicitly enabled.
- Make telemetry opt-in. Local CLI analysis must work without telemetry.
- Treat automatic identity matches as reviewable suggestions. Destructive
  merges require explicit confirmation and must be auditable.
- Represent unavailable or insufficient data explicitly; never convert it to
  a zero that could be misread as poor performance.

## CLI and Agent Contracts

- Keep human-readable output and logs separate from machine output: result
  data goes to stdout and diagnostics go to stderr.
- Support non-interactive execution, stable exit codes, `--no-color`, and a
  versioned JSON schema.
- Include analysis version, rule-set hash, source revision, metric provenance,
  warnings, and data sufficiency in machine output.
- Preserve backward compatibility within a schema major version.
- Agent Skills authenticate to hosted Assay with scoped, revocable Assay API
  tokens. They must not receive browser session cookies or upstream OIDC
  refresh tokens.
- The Agent Skill must explain limitations and link aggregate observations to
  change sets or source records. It must not infer employee performance,
  compensation decisions, or intent from repository activity.
- Project-intelligence summaries and AI rubric judgments must cite evidence.
  Language models may summarize evidence and rate bounded qualitative
  criteria, but a deterministic, versioned compiler produces published scores.
- A local or trusted single-operator installation may reuse an existing Codex
  CLI login. The public multi-tenant service uses server-managed API
  credentials by default and may offer the explicitly experimental, isolated
  Codex OAuth broker defined in the Project Intelligence specification.
- Never invent undocumented Codex OAuth endpoints or ask users to paste token
  material. Keep OAuth support feature-flagged, tenant-isolated, encrypted,
  revocable, auditable, and removable through a server-side kill switch.

## Dependency and Generated-File Policy

- Keep dependencies minimal and justify new runtime services.
- Commit lockfiles appropriate to each build system.
- Do not hand-edit generated code or generated schemas. Change the generator
  or source definition and regenerate.
- Do not commit build output, local caches, tokens, private repository data, or
  benchmark clones.
