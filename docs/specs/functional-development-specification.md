# Assay Functional Development Specification

- Status: Draft
- Product language: English
- Initial delivery surface: CLI
- Planned delivery surfaces: Agent Skill, REST API, web dashboard
- Primary implementation: Rust, PostgreSQL, Next.js with TypeScript
- Source research: `.orca/drops/contribution-metrics-handoff.md`

## 1. Purpose

Assay is an open-source tool that extracts reviewable contribution signals
from Git repositories and development platforms. It reports activity,
semantic change, durability, collaboration, and maintenance signals without
converting them into a developer performance score.

The first product is a deterministic CLI for JavaScript, TypeScript, and
Python repositories. The same analysis engine will later power an Agent Skill,
a hosted API and worker, and a web dashboard.

This document defines product behavior, implementation boundaries, functional
requirements, machine contracts, acceptance criteria, and delivery phases.

Project-level open-source evaluation and discovery are a separate bounded
domain specified in
`docs/specs/open-source-project-intelligence-specification.md`. Its transparent
project scores do not relax this document's prohibition on person-level
performance scoring.

## 2. Normative Terms and Priorities

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

| Priority | Meaning |
| --- | --- |
| P0 | Required for the initial public CLI release |
| P1 | Required for the first useful Agent Skill or hosted release |
| P2 | Planned extension after the core metrics have been validated |

## 3. Product Positioning

### 3.1 Product promise

Assay provides honest, comparable reference signals with their provenance,
uncertainty, and limitations. It helps a person or team inspect trends, start
retrospective conversations, and find changes that deserve attention.

The guiding measurement principle is:

> Measure changes that were accepted, delivered, and remained healthy, while
> preserving the context needed to explain them.

### 3.2 Non-goals

Assay MUST NOT:

- claim to measure a person's value, effort, productivity, or business impact;
- emit a single composite contribution or performance score;
- provide a global developer leaderboard;
- make compensation, promotion, termination, or hiring recommendations;
- detect or accuse a contributor of using AI;
- treat commit count or lines of code as a standalone outcome;
- treat churn as evidence of poor performance;
- hide the source change sets behind an unexplained aggregate;
- silently merge identities or silently classify a human as a bot; or
- assume that unavailable data is equivalent to a zero value.

### 3.3 Design principles

1. A merged pull request or logical change set is the default unit of work.
2. Raw observations and derived metrics are stored separately.
3. Every derived metric is reproducible from a versioned rule set.
4. Noise is classified or moved to an appropriate dimension, not blindly
   discarded.
5. Trends are the default view; comparisons are contextual and opt-in.
6. Every aggregate supports drill-down to its evidence.
7. Uncertainty and insufficient data are first-class output states.
8. Collection scope and classification rules are visible and reviewable.

## 4. Users and Core Use Cases

### 4.1 User roles

| Role | Need |
| --- | --- |
| Contributor | Inspect personal trends and correct identity or context errors |
| Maintainer | Understand repository health, review load, and durable changes |
| Team facilitator | Prepare a retrospective without ranking individuals |
| Researcher | Export reproducible observations and benchmark metric behavior |
| Agent | Run a bounded analysis and explain evidence through a stable CLI |
| Administrator | Configure repositories, identities, privacy, and collection |

### 4.2 Primary use cases

- Analyze a local repository over a selected revision and time range.
- Analyze a public or authorized GitHub repository by pull request.
- Separate generated, vendored, formatting-only, and dependency changes from
  semantic production-code changes.
- Measure whether accepted code was modified, reverted, or removed within a
  configurable time window.
- Export evidence and metrics as JSON, JSON Lines, or CSV.
- Explain how a metric was calculated and open the contributing change sets.
- Correct identity aliases and attach human context to an unusual trend.
- Let an Agent Skill answer questions using evidence without inventing an
  overall performance judgment.

## 5. Product Modes

### 5.1 Local CLI mode — P0

Local mode analyzes a working tree or bare Git repository without requiring a
server. It MUST support deterministic output and MUST NOT require telemetry or
a PostgreSQL instance.

### 5.2 Connected CLI mode — P0/P1

Connected mode enriches Git history with GitHub pull requests, reviews,
issues, checks, and deployment records. Public repositories and authorized
private repositories are supported. API responses are cached incrementally.

### 5.3 Agent Skill mode — P1

The Agent Skill invokes the installed CLI in non-interactive mode, validates
its versioned JSON output, and explains the results with the required product
limitations. It contains no independent metric implementation.

### 5.4 Hosted mode — P1

Hosted mode uses PostgreSQL, a Rust API, and asynchronous workers. It supports
scheduled synchronization, webhook ingestion, analysis jobs, annotations,
and the web dashboard.

## 6. System Architecture

```text
Local Git / GitHub API / GitHub Webhooks
                    |
                    v
       Git and platform collectors
                    |
                    v
       Identity and change-set resolver
                    |
                    v
         File policy classifier
                    |
                    v
     Pluggable semantic-diff analysis
                    |
                    v
     Outcome and durability correlation
                    |
                    v
       Versioned metric computation
          |                    |
          v                    v
 CLI JSON/CSV/human      PostgreSQL storage
                               |
                         Rust API/worker
                               |
                        Next.js dashboard
```

### 6.1 Component responsibilities

| Component | Responsibility |
| --- | --- |
| `assay-domain` | Entities, value objects, invariants, statuses, and ports |
| `assay-git` | Git traversal, revision snapshots, blame/survival facts, change sets |
| `assay-github` | Pull requests, reviews, issues, checks, deployments, and webhooks |
| `assay-classifier` | Linguist-compatible file and change classification |
| `assay-semantic-diff` | AST parsing, matching, and structural change categories |
| `assay-metrics` | Versioned metric definitions, normalization, and sufficiency |
| `assay-storage` | File-cache, optional local, and PostgreSQL adapters |
| `assay-cli` | Human and machine-facing command-line interface |
| `assay-api` | Authenticated REST resources and analysis job control |
| `assay-worker` | Backfill, synchronization, analysis, and recomputation jobs |
| `web` | Report composition, trend exploration, context, and drill-down |
| `skills/assay` | Thin agent workflow around the public CLI contract |

## 7. Functional Requirements

### 7.1 Repository intake and collection

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| COL-001 | P0 | Accept a local working tree or bare repository. | A fixture can be analyzed at the current `HEAD` or explicit revision without network access. |
| COL-002 | P0 | Accept a GitHub `owner/repository` or URL when credentials permit. | Public repositories work anonymously within platform limits; private access uses environment or credential-provider integration without printing tokens. |
| COL-003 | P0 | Support `--since`, `--until`, base revision, and head revision boundaries. | The output records the resolved commit IDs and normalized UTC interval. |
| COL-004 | P0 | Cache Git and GitHub collection incrementally. | Repeating an unchanged analysis does not refetch or reparse unchanged inputs. |
| COL-005 | P0 | Record partial collection and rate-limit warnings. | Partial output is marked `partial`; missing sources and retry guidance are machine-readable. |
| COL-006 | P1 | Receive and verify GitHub webhooks. | Invalid signatures are rejected; delivery IDs are idempotent; accepted events enqueue reconciliation. |
| COL-007 | P1 | Reconcile webhook state with periodic API backfill. | Missed or out-of-order events converge to the source state without duplicating facts. |

### 7.2 Change-set formation

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| CHG-001 | P0 | Use the final merged pull request as the preferred unit. | Squash, merge-commit, and rebase strategies resolve to one stable change-set record when PR data is available. |
| CHG-002 | P0 | Form a logical fallback change set without PR metadata. | The output identifies the fallback strategy and lists every included commit. |
| CHG-003 | P0 | Preserve base, head, and final merge revisions. | Every analyzed change set is reproducible against immutable commit IDs. |
| CHG-004 | P0 | Attach authors, co-authors, reviewers, linked issues, files, and available outcomes. | Missing relations are `unavailable`, not empty facts presented as zero activity. |
| CHG-005 | P1 | Link checks, deployments, reverts, hotfixes, and follow-up changes. | Each link includes source, match method, confidence, and timestamp. |

### 7.3 Identity resolution

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| IDN-001 | P0 | Model GitHub login, author name, email, co-author trailer, and bot identity as aliases. | A person can have multiple aliases without losing original attribution. |
| IDN-002 | P0 | Apply explicit configuration before heuristic matching. | User mappings are deterministic and include an audit source. |
| IDN-003 | P0 | Mark heuristic matches with confidence and evidence. | Suggested matches never become irreversible merges automatically. |
| IDN-004 | P0 | Provide list, link, unlink, and bot-override operations. | Changes are reviewable in dry-run form and persisted only after explicit confirmation. |
| IDN-005 | P1 | Provide a self-correction workflow in hosted mode. | A contributor can inspect aliases and submit a correction with an audit trail. |

### 7.4 File and change classification

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| CLS-001 | P0 | Respect `.gitattributes` and Linguist generated/vendored conventions. | Repository overrides are applied and included in provenance. |
| CLS-002 | P0 | Detect common lockfiles, build output, coverage, vendored code, generated code, and minified files. | The default rules cover the research handoff list and can be overridden in configuration. |
| CLS-003 | P0 | Classify production, test, documentation, CI/CD, infrastructure, schema migration, dependency, security, and configuration changes. | Every changed file has one primary category, optional tags, rule ID, and confidence. |
| CLS-004 | P0 | Move dependency and operations work to maintenance signals rather than dropping it. | A lockfile-only security update has low semantic-code impact but remains visible as maintenance evidence. |
| CLS-005 | P0 | Detect whitespace-only and format-only changes. | Golden fixtures show zero semantic operations for supported format-only edits while retaining raw change facts. |
| CLS-006 | P1 | Classify feature, bug fix, refactor, chore, test, and documentation work. | Classification reports evidence and confidence; uncertain cases remain `unknown`. |

### 7.5 Semantic diff

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| SEM-001 | P0 | Parse JavaScript, TypeScript, TSX, and Python through tree-sitter-compatible parsers. | Supported fixtures produce parser version, language, errors, and syntax-tree facts. |
| SEM-002 | P0 | Report added, removed, modified, moved, and renamed semantic units. | Function/class/component fixtures distinguish moves or renames from delete-plus-add where the selected engine can do so. |
| SEM-003 | P0 | Separate raw line changes from semantic operations. | Both are exported; no metric silently substitutes one for the other. |
| SEM-004 | P0 | Expose a pluggable engine interface. | The spike can compare native tree-sitter matching with GumTree or difftastic-derived behavior using identical fixtures. |
| SEM-005 | P0 | Handle parse errors without discarding the change set. | The affected file falls back to a declared text-level result and the run becomes partial, not silently successful. |
| SEM-006 | P1 | Calculate function-level complexity delta and affected API/schema units. | Results identify parser/analyzer version and are absent when unsupported. |
| SEM-007 | P2 | Estimate call-graph impact for supported ecosystems. | The output is labeled as static approximation and links to the discovered symbols. |

### 7.6 Churn and durability

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| DUR-001 | P0 | Calculate modification or deletion within configurable windows. | Defaults include a 14-day window; users can configure additional day-based windows. |
| DUR-002 | P0 | Calculate survival against immutable revisions. | Re-running the same revision and rules produces identical results. |
| DUR-003 | P0 | Label churn as rework/risk evidence. | CLI help, schemas, and human reports contain no productivity or penalty language. |
| DUR-004 | P1 | Correlate reverts, hotfixes, defect issues, CI outcomes, and deployments. | Every correlation exposes source and match confidence. |
| DUR-005 | P1 | Support 30-day and 60-day durability maturation. | A metric not old enough is `pending_maturation`, not successful durability. |

### 7.7 Metric vectors

Assay exposes observations in five independent dimensions. A dimension may be
`available`, `partial`, `insufficient_data`, `unsupported`, or
`pending_maturation`.

| Dimension | Initial signals | Priority |
| --- | --- | --- |
| Delivery | Merged PRs and PR lead time; deployed PRs and recovery signals when integrations exist | P0/P1 |
| Semantic Impact | Changed semantic units, change categories, complexity delta, API and schema changes | P0 |
| Durability / Quality | Churn windows, survival, reverts, hotfixes, CI and defect outcomes | P0/P1 |
| Collaboration / Leverage | Reviews, accepted suggestions, issue unblocking, ownership distribution | P1 |
| Maintenance / Risk Reduction | Dependencies, security, CI stability, observability, flaky-test and debt work | P0/P1 |

All metric definitions MUST include:

- a stable metric ID and definition version;
- unit, population, window, and aggregation method;
- required data sources and sufficiency rule;
- the raw observation IDs used in the result;
- limitations and common misinterpretations;
- analyzer version and rule-set hash; and
- availability or uncertainty status.

### 7.8 Normalization and comparison

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| NRM-001 | P1 | Calculate within-repository percentiles or robust z-scores by language and work type. | Raw and normalized values remain separate and the cohort definition is exported. |
| NRM-002 | P1 | Provide scale-free ratios such as churn rate and survival rate as separate axes. | Ratios disclose denominator and minimum sample requirements. |
| NRM-003 | P1 | Refuse misleading small-sample normalization. | Cohorts below the configured threshold emit `insufficient_data` and no percentile. |
| NRM-004 | P2 | Aggregate repository-relative observations across repositories. | The UI and API retain per-repository values and do not emit a universal score. |
| NRM-005 | P2 | Support stratified cohorts by language, lifecycle stage, role, and work type. | Cohort membership is explicit, configurable, and auditable. |

### 7.9 Reporting, explanation, and context

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| RPT-001 | P0 | Export human, JSON, JSON Lines, and CSV output. | JSON validates against the declared schema; CSV has documented stable columns. |
| RPT-002 | P0 | Drill from a metric to change sets and source links. | Every aggregate returns evidence IDs; connected GitHub records include canonical URLs. |
| RPT-003 | P0 | Explain a metric's definition, inputs, exclusions, and warnings. | `assay explain` works without recalculating an unchanged run. |
| RPT-004 | P0 | Display trends before contributor comparisons. | Default human reports show time buckets and no rank column. |
| RPT-005 | P1 | Attach annotations and event overlays. | Annotations record author, time, scope, and revision history. |
| RPT-006 | P1 | Show releases, incidents, leave, and organizational events when supplied. | Context is visually and structurally separate from measured facts. |
| RPT-007 | P1 | Apply k-anonymity to team aggregate views. | Groups below configured `k` are suppressed with an explicit reason. |

### 7.10 Configuration and reproducibility

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| CFG-001 | P0 | Load versioned `assay.toml` configuration. | Unknown keys fail validation by default; effective config can be printed with secrets redacted. |
| CFG-002 | P0 | Externalize file rules, churn windows, identities, and privacy options. | A configuration change changes the rule-set hash. |
| CFG-003 | P0 | Record an immutable analysis manifest. | The manifest includes source revisions, interval, config hash, analyzer and parser versions, and data-source status. |
| CFG-004 | P0 | Recompute derived metrics without recollecting unchanged raw facts. | A metric version change creates a new analysis result and preserves the prior result. |
| CFG-005 | P1 | Support repository, organization, and deployment configuration layers. | Effective precedence is deterministic and inspectable. |

## 8. CLI Contract

### 8.1 Commands

| Command | Pri | Purpose |
| --- | --- | --- |
| `assay analyze <source>` | P0 | Collect and analyze a repository or revision |
| `assay report <run>` | P0 | Render an existing analysis in another format |
| `assay explain <run>` | P0 | Explain metrics, warnings, and evidence |
| `assay export <run>` | P0 | Write versioned JSON, JSONL, or CSV artifacts |
| `assay config init` | P0 | Generate a documented default configuration |
| `assay config check` | P0 | Validate and print the effective configuration |
| `assay identity list` | P0 | Show identities, aliases, evidence, and confidence |
| `assay identity link` | P0 | Add an explicit alias mapping with dry-run support |
| `assay identity unlink` | P0 | Remove an explicit mapping without deleting facts |
| `assay capabilities` | P0 | Report supported languages, engines, schemas, and integrations |
| `assay schema` | P0 | Print or write a bundled machine-readable schema |
| `assay sync` | P1 | Incrementally synchronize a configured connected repository |

### 8.2 Common options

The CLI MUST support:

```text
--config <path>
--format human|json|jsonl|csv
--output <path|->
--since <timestamp|revision>
--until <timestamp|revision>
--no-color
--quiet
--non-interactive
--log-level <level>
```

Machine result data MUST go to stdout when `--output -` is used. Logs,
progress, and warnings MUST go to stderr. Machine formats MUST never contain
ANSI control sequences.

### 8.3 Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Complete or explicitly represented partial result was produced |
| 2 | Invalid command, input, or configuration |
| 3 | Authentication or authorization failure |
| 4 | Source or revision not found |
| 5 | Retryable platform or rate-limit failure with no usable result |
| 10 | Collection failure |
| 11 | Analysis failure |
| 12 | Output or schema validation failure |

Insufficient data is a result status, not a process failure.

### 8.4 Machine envelope

All JSON results use a stable envelope resembling:

```json
{
  "schema_version": "1.0.0",
  "tool": { "name": "assay", "version": "0.1.0" },
  "run": {
    "id": "content-derived-or-persisted-id",
    "status": "complete",
    "source_revision": "full-commit-id",
    "rule_set_hash": "sha256-value"
  },
  "data_sources": [],
  "subjects": [],
  "change_sets": [],
  "metrics": [],
  "warnings": [],
  "limitations": []
}
```

The canonical schema belongs in `schemas/`. Fields may be added compatibly
within major version 1, but existing meanings MUST NOT change.

## 9. Agent Skill Contract

### 9.1 Packaging

The planned skill package is `skills/assay/` and contains:

```text
skills/assay/
  SKILL.md
  agents/openai.yaml
  scripts/          # only deterministic CLI invocation helpers, if needed
  references/       # CLI and metric/schema references loaded on demand
```

The skill MUST remain concise. Large metric definitions and schemas belong in
references or the product documentation, not duplicated in `SKILL.md`.

### 9.2 Trigger examples

The skill should trigger for requests such as:

- "Analyze durable contribution signals in this repository."
- "Which merged changes were substantially reworked within 30 days?"
- "Explain the semantic and maintenance work in this sprint."
- "Export Assay metrics for these pull requests as JSON."
- "Check whether this trend has enough data to interpret."

It should not trigger for generic code review, employee evaluation, or GitHub
administration that does not require Assay analysis.

### 9.3 Agent workflow

The skill instructs an agent to:

1. Confirm the requested repository, revision or interval, and allowed data
   source from available context.
2. Run `assay capabilities --format json` when compatibility is unknown.
3. Validate configuration with `assay config check`.
4. Run the narrowest applicable non-interactive CLI command.
5. Parse only schema-validated machine output.
6. Report data sufficiency, warnings, and unavailable sources before making an
   interpretation.
7. Explain independent metric dimensions and link them to evidence.
8. State that the result is a reference signal and not a performance score.

### 9.4 Agent guardrails

The Agent Skill MUST NOT:

- invent a missing metric or replace missing data with zero;
- combine dimensions into an overall score;
- rank contributors unless a future, explicitly requested safe view is defined;
- attribute intent, AI usage, competence, or effort from repository evidence;
- automatically accept heuristic identity matches;
- expose private source, emails, tokens, or raw payloads in its response; or
- mutate GitHub, post comments, or change repository state during analysis.

The skill SHOULD prefer bounded summaries and offer evidence drill-down when
the result is large. CLI JSON remains the source of truth.

## 10. Hosted API, Worker, and Web Requirements

### 10.1 API — P1

- Provide versioned REST endpoints under `/api/v1`.
- Expose repositories, sync state, analysis runs, metrics, evidence,
  identities, annotations, configuration summaries, and exports.
- Use asynchronous job resources for collection and analysis.
- Return stable error codes and correlation IDs.
- Enforce repository- and organization-scoped authorization.
- Generate an OpenAPI contract and test it against the implementation.

### 10.2 Worker — P1

- Perform GitHub backfill, webhook reconciliation, semantic analysis,
  maturation checks, and metric recomputation.
- Use idempotent jobs keyed by repository, revision, analyzer version, and
  rule-set hash.
- Begin with a PostgreSQL-backed job lease before introducing a separate queue
  service.
- Apply bounded concurrency and platform-aware rate limiting.
- Preserve failed job diagnostics without logging secrets or private source.

### 10.3 Web dashboard — P1

- Use Next.js with TypeScript.
- Fetch product data through the Rust API. Do not duplicate metric formulas or
  write independently to the Assay database.
- Render summary pages as Server Components where practical; isolate charts
  and filters as Client Components.
- Default to trend and small-multiple views rather than ranked tables.
- Visualize confidence, maturation, and insufficient-data states.
- Support drill-down from an observation to pull requests and change sets.
- Provide annotation, identity correction, export, and metric explanation
  workflows.
- Meet WCAG 2.2 AA for the supported user flows.

## 11. Persistence Model

The server-mode PostgreSQL model SHOULD contain the following conceptual
records:

| Record | Purpose |
| --- | --- |
| Repository | Source identity, provider, visibility, and collection policy |
| SourceSnapshot | Immutable revisions and time boundaries |
| RawEvent | Provider payload metadata and content-addressed provenance |
| ChangeSet | Pull request or logical unit with base/head/merge revisions |
| Identity | Person or bot subject without destroying original aliases |
| IdentityAlias | Login, name, email hash, trailer, mapping evidence, confidence |
| FileChange | Raw diff statistics, path, language, and file classification |
| SemanticChange | Structural unit and operation produced by a named engine |
| OutcomeEvent | Check, deployment, revert, hotfix, issue, or incident relation |
| AnalysisRun | Immutable manifest of source, configuration, and tool versions |
| MetricObservation | Versioned value, status, cohort, uncertainty, and evidence |
| Annotation | Human context with author, scope, and revision history |

Raw facts MUST be append-only or historically recoverable. Recalculation MUST
create new derived observations instead of rewriting the meaning of a prior
result.

PostgreSQL is the authoritative hosted store. Local CLI mode MAY use a
content-addressed file cache or a limited embedded database, but portable
server behavior only needs to support PostgreSQL.

## 12. Privacy, Trust, and Ethical Requirements

| ID | Pri | Requirement |
| --- | --- | --- |
| TRU-001 | P0 | Include prohibited-use and interpretation guidance in CLI help, schemas, and public documentation. |
| TRU-002 | P0 | Make telemetry opt-in and allow fully local execution. |
| TRU-003 | P0 | Make collection scope and effective classification rules inspectable. |
| TRU-004 | P0 | Redact credentials and avoid retaining full source or raw diffs by default. |
| TRU-005 | P1 | Let contributors inspect their attributed evidence and request corrections. |
| TRU-006 | P1 | Provide anonymized or pseudonymized exports. |
| TRU-007 | P1 | Suppress group statistics below a configurable k-anonymity threshold. |
| TRU-008 | P1 | Keep an audit log for identity, annotation, configuration, and access changes. |
| TRU-009 | P2 | Support configurable retention and deletion for hosted personal data. |

## 13. Non-functional Requirements

### 13.1 Determinism and reproducibility

- Identical source revisions, configuration, parser versions, and analyzer
  versions MUST produce identical semantic and metric output.
- Non-deterministic provider fields MUST be isolated from calculation inputs.
- Timestamps MUST use RFC 3339 UTC in machine output.
- Stable IDs SHOULD be content-derived where feasible.

### 13.2 Performance

- Analysis MUST be incremental at the file and change-set level.
- The CLI MUST stream progress to stderr without buffering full repositories in
  memory.
- Concurrency MUST be configurable and bounded.
- Performance benchmarks MUST report repository size, commit count, file
  count, cache state, hardware, and tool version.
- Initial numeric performance budgets will be set only after the semantic-diff
  spike establishes a reproducible baseline.

### 13.3 Reliability

- Network operations use bounded retries with exponential backoff and jitter.
- Webhook deliveries and worker jobs are idempotent.
- A failure in one file parser does not erase otherwise valid change-set facts.
- Partial results identify their missing sources and affected metrics.

### 13.4 Portability

- The CLI SHOULD ship as a self-contained binary for major Linux, macOS, and
  Windows targets when parser dependencies allow it.
- Local analysis MUST work in CI and headless agent environments.
- Machine contracts MUST not depend on terminal width or locale.

### 13.5 Observability

- Use structured logs with run, repository, job, and correlation IDs.
- Expose durations, cache hits, API budget, parser failures, queue depth, and
  metric maturation counts in hosted mode.
- Logs MUST NOT include access tokens, private source text, or unredacted email
  addresses.

## 14. Validation Strategy

### 14.1 Required fixture classes

The test suite MUST include synthetic histories for:

- whitespace-only and formatter-only changes;
- function and file rename or move;
- copied code versus new logic;
- generated, vendored, minified, and lockfile changes;
- test, documentation, CI, infrastructure, migration, and security changes;
- merge commit, squash merge, rebase merge, revert, and hotfix patterns;
- code modified or deleted across each churn boundary;
- multiple emails, GitHub aliases, co-author trailers, squash authors, and bots;
- parser errors and unsupported languages;
- missing reviews, deployments, or insufficient cohort data; and
- API pagination, rate limiting, duplicated webhooks, and partial collection.

### 14.2 Semantic-diff spike exit criteria

Before choosing the first production engine, compare native tree-sitter
matching, difftastic-derived behavior, and GumTree where integration is
feasible. The spike records:

- correctness against reviewed golden fixtures;
- move, rename, copy, and formatting behavior;
- parser coverage and error handling;
- cold and warm performance;
- memory consumption;
- licensing and distribution consequences; and
- integration complexity in a Rust CLI.

The decision is recorded as an architecture decision. Spike code that is not
selected is removed or clearly isolated from production.

### 14.3 Metric validation

- Publish definitions before publishing benchmark conclusions.
- Compare metric output with independently reviewed change-set labels.
- Report false positives, false negatives, confidence, and known biases.
- Use public, redistributable benchmark repositories and pin immutable
  revisions.
- Never validate a metric only by correlation with LOC or commit count.

### 14.4 Contract testing

- Validate all machine output against JSON Schema.
- Maintain reviewed golden files for CLI and Agent Skill examples.
- Test additive compatibility within schema major versions.
- Test API behavior against generated OpenAPI.

## 15. Delivery Plan

### Phase 0 — Foundation and spike

- Establish the Rust workspace and continuous integration.
- Define domain entities, analysis manifest, file policy, and output schema.
- Build reviewed synthetic Git histories.
- Compare semantic-diff engines and record the decision.

Exit condition: the project can deterministically classify and structurally
compare the supported fixture changes.

### Phase 1 — Public CLI MVP

- Implement local Git analysis for JavaScript, TypeScript, TSX, and Python.
- Implement change-set formation, identity aliases, file classification,
  format-only detection, semantic operations, and configurable churn.
- Ship human, JSON, JSONL, and CSV reports with explain and drill-down data.
- Publish metric definitions, prohibited uses, and reproducible examples.

Exit condition: `assay analyze` passes golden and schema tests on supported
fixtures and selected pinned public repositories.

### Phase 2 — GitHub enrichment and Agent Skill

- Add pull requests, reviews, issues, checks, and deployment enrichment.
- Add incremental cache behavior and explicit partial-data states.
- Finalize the stable CLI JSON contract and capability discovery.
- Create and validate the thin Agent Skill against realistic requests.

Exit condition: the Agent Skill answers bounded questions from validated CLI
output and consistently includes evidence, uncertainty, and limitations.

### Phase 3 — Hosted service and dashboard

- Add PostgreSQL migrations, API, worker, webhooks, and reconciliation.
- Add the Next.js trend, drill-down, identity correction, annotation, and
  export workflows.
- Add k-anonymity, access control, audit events, and retention configuration.

Exit condition: a repository can be synchronized incrementally and inspected
end to end without duplicating business logic in the web application.

### Phase 4 — Validation and expansion

- Add collaboration outcomes, 30/60-day maturation, and deployment/defect
  correlations.
- Add repository-relative normalization with small-sample protection.
- Add languages only after parser and fixture quality meets the published
  support bar.
- Publish reproducible benchmark and validation datasets.

## 16. Definition of Done

A functional requirement is done only when:

- its behavior is implemented behind the intended architecture boundary;
- unit, fixture, integration, and contract tests appropriate to the change
  pass;
- machine output is schema-valid and versioned;
- failure, partial, unsupported, and insufficient-data behavior is tested;
- provenance and security implications are addressed;
- public documentation and metric limitations are updated in English;
- format, lint, test, and build checks pass; and
- no generated output, credentials, private data, or benchmark clones are
  included in the change.

## 17. Open Decisions

The following decisions require spikes or explicit project approval:

1. Native tree-sitter matching versus GumTree or difftastic-derived semantic
   matching.
2. The exact logical change-set fallback when pull request data is absent.
3. Local cache format and whether a limited SQLite adapter is worthwhile.
4. Git history implementation: Git CLI, `gix`, or `libgit2` adapter.
5. Initial public metric formulas and minimum sample thresholds.
6. Rules for linking defects, hotfixes, deployments, and review suggestions.
7. Open-source license and dependency-license policy.
8. Authentication provider and tenancy model for hosted mode.
9. Final visualization library after trend and uncertainty prototypes.
10. Distribution targets and installation channels for the CLI and Agent Skill.

## 18. Repository and Release Governance

- All public project artifacts are written in English.
- Releases include CLI binaries, checksums, schema artifacts, metric-definition
  versions, and dependency/license notices.
- Breaking CLI, schema, metric-definition, or configuration changes require an
  explicit migration note and appropriate version increment.
