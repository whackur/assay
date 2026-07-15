# Assay Open Source Project Intelligence Specification

- Status: Draft
- Product language: English
- Initial surfaces: CLI, local dashboard, server analysis jobs, and public
  project catalog
- Planned surface: Agent Skill
- Related specifications: `functional-development-specification.md`,
  `identity-private-workspace-and-entitlements-specification.md`

## 1. Purpose

Assay Project Intelligence evaluates the substance, originality, engineering
quality, open-source readiness, maintenance health, and potential of an
open-source project. Hosted catalog analysis accepts public GitHub
repositories. A local installation may also analyze an authorized private
GitHub repository without publishing its source or result.

The product accepts a repository URL or project identifier, gathers public
evidence, performs reproducible analysis, calculates explainable project-level
scores, automatically discovers functionally similar projects, and produces an
evidence-grounded introduction for a discovery catalog.

The evaluation is authorship-agnostic. AI-assisted development is neither a
positive nor a negative signal. Assay evaluates whether the resulting project
has meaningful implementation, demonstrable utility, sound engineering, and
credible potential.

## 2. Relationship to Contribution Intelligence

Contribution Intelligence and Project Intelligence share collectors,
classifiers, semantic-diff facts, durability analysis, schemas, storage, and
delivery infrastructure. They have different subjects and policies.

| Domain | Subject | Output policy |
| --- | --- | --- |
| Contribution Intelligence | Change set, contributor, or team trend | Independent reference signals; no person-level composite score |
| Project Intelligence | Public open-source project | Explainable dimensions, optional overall project score, potential, and introduction |

Project scores MUST NOT be reused to evaluate an individual maintainer or
contributor. Person-level activity MUST NOT enter a project score except as an
aggregated project-health fact such as contributor diversity or maintainer
concentration.

## 3. Product Principles

1. Evaluate implemented substance rather than presentation volume.
2. Verify claims against source, tests, releases, demos, and history.
3. Separate current value from future potential.
4. Make every score traceable to versioned rules and timestamped evidence.
5. Treat small and young projects fairly through confidence and maturity.
6. Recognize legitimate templates, forks, generated code, and licensed reuse.
7. Never infer author intent from weak repository signals.
8. Provide a simple contact path for factual or provenance concerns without
   allowing manual score edits.
9. Keep editorial promotion independent from analytical scores.
10. Preserve prior score snapshots when rules or evidence change.

## 4. Goals and Non-goals

### 4.1 Goals

- Let a user submit only a public repository URL and receive a useful profile.
- Make the first value moment immediately legible: “How does my project
  measure up?”
- Distinguish meaningful projects from hollow, misleading, minimally altered,
  or unvalidated repositories.
- Identify originality without confusing legitimate reuse with imitation.
- Evaluate static, historical, and reported-CI evidence of whether the project
  is installable, testable, maintainable, and credible for its declared
  maturity without executing repository code.
- Automatically find and compare a bounded set of projects with a similar
  purpose and feature set.
- Estimate future potential with explicit assumptions and confidence.
- Generate a concise introduction explaining who the project is for and why it
  matters.
- Build a browsable catalog of notable open-source projects.
- Track how project quality, health, and potential change over time.

### 4.2 Non-goals

Assay Project Intelligence MUST NOT:

- detect or estimate whether code was written by AI;
- label a maintainer as lazy, malicious, fraudulent, or unskilled;
- accuse a project of plagiarism based only on similarity;
- treat stars, forks, downloads, commits, or LOC as standalone quality;
- use popularity as a direct or material score contribution;
- penalize a young project merely for having little history;
- treat unavailable evidence as a numeric zero;
- let an LLM directly choose a published score;
- guarantee future adoption, commercial success, or security;
- publish private repository data in the public catalog; or
- show an overall score without dimensions, evidence, confidence, and version.

## 5. Core User Journey

```text
GitHub URL entered in the web form
                |
                v
 Canonicalize repository and resolve HEAD
                |
                v
 Check evaluation cache and in-flight job
       | cache hit       | cache miss
       v                 v
 Return result    Reserve job capacity
                             |
                             v
          Collect public facts and run analysis
                             |
                             v
             AI evidence-rubric evaluation
                             |
                             v
        Compile versioned scores and confidence
                             |
                             v
 Generate grounded introduction, comparisons, and card
                             |
                             v
       Store an engine-specific immutable snapshot
               | anonymous | authenticated
               v           v
        publish result   private preview
```

## 6. Inputs and Evidence

### 6.1 Accepted input

The system MUST accept one of:

- a canonical GitHub repository URL;
- a GitHub `owner/repository` identifier;
- a local clone for private analysis without catalog publication; or
- a private GitHub URL in local mode when a token is obtained only from a
  named environment variable.

The initial hosted service accepts GitHub public repositories only. GitLab,
other forges, and hosted private-repository connections are deferred. A public
web form MUST NOT accept a GitHub personal access token.

### 6.2 Public evidence sources

The analyzer MAY collect:

- repository metadata, topics, license, default branch, and archive status;
- immutable Git history, tags, releases, and source tree;
- README, documentation, examples, changelog, roadmap, contribution guide,
  security policy, and architecture documents;
- issues, discussions, pull requests, reviews, and response intervals;
- CI workflows, CI-reported test execution, coverage metadata, static analysis,
  dependency updates, release automation, and build instructions;
- package registries, release artifacts, downstream dependencies, and public
  adoption evidence;
- a project website, documentation site, demonstration, paper, or announcement
  explicitly linked by the repository; and
- OpenSSF Scorecard structured checks or comparable security evidence.

Every fact MUST record its source, collection time, immutable revision or
remote identifier where possible, and availability status.

Assay MUST NOT install dependencies, build, test, import, or execute analyzed
repository code. Reported CI state and static configuration are evidence;
Assay does not claim independent build verification.

### 6.3 Evidence grades

| Grade | Description |
| --- | --- |
| A | Immutable source, reported CI, or release fact pinned to a revision |
| B | Structured platform or package-registry fact with stable provenance |
| C | Maintainer claim supported by a linked artifact or demonstration |
| D | Unverified textual claim or weak popularity proxy |

Scores SHOULD prefer grades A and B. Grade D may reveal a mismatch between a
claim and implementation but MUST NOT establish positive project quality by
itself.

## 7. Project Classification

Scoring begins by classifying project type and maturity.

### 7.1 Project type

- application or end-user product;
- library, SDK, or framework;
- CLI or developer tool;
- service, infrastructure, or platform;
- curated resource or awesome list;
- protocol, specification, or standard;
- dataset, model, or research artifact;
- educational example or template; or
- experimental proof of concept.

A project has one primary type and MAY have secondary types and descriptive
tags. A CLI, library, or framework label determines applicable rubric rules;
it does not by itself define the comparison cohort.

### 7.2 Maturity

- concept;
- prototype;
- alpha;
- beta;
- stable;
- maintenance;
- dormant; or
- archived.

Expectations and rule applicability vary by type and maturity. A clearly
labeled educational template is not low-substance merely because it is a
template. Presenting the same template as a complete original product creates
a claim-to-implementation mismatch.

### 7.3 Functional comparison cohort

Assay constructs comparison cohorts from the problem addressed, target user,
implemented features, and technical approach. Projects MAY be comparable even
when their delivery forms differ, and projects of the same delivery form MUST
NOT be compared when they solve unrelated problems.

An awesome list is evaluated as a curated artifact. Its linked projects are
not recursively analyzed. Similarity compares it with other curated lists in
the same topic using list structure, overlap, unique coverage, editorial
quality, and maintenance evidence.

## 8. Evaluation Dimensions

Each dimension uses a 0–100 scale, 0–1 confidence, status, evaluation version,
and positive and negative evidence. Criteria are `applicable`,
`partially_applicable`, or `not_applicable` for the classified project type and
maturity. `Insufficient evidence` and `not_applicable` are statuses, not zero
scores.

### 8.1 Project Substance — 0–100

Project Substance measures whether the repository contains a meaningful,
working implementation of its stated purpose.

Positive evidence includes:

- executable domain behavior beyond initial scaffolding;
- stated features that correspond to code and tests;
- examples or demonstrations that exercise real behavior;
- implementation depth appropriate to the project's declared scope;
- meaningful iteration after the initial import;
- resolved defects and incorporated user feedback; and
- coherent source, configuration, documentation, and release artifacts.

Negative evidence includes:

- mostly untouched framework scaffolding or generated output;
- placeholder tests, TODO implementations, hard-coded demonstrations, or dead
  paths presented as finished features;
- extensive claims with little corresponding implementation;
- reported non-working installation, examples, CI builds, or published
  packages;
- a one-shot code dump with no validation or meaningful iteration; and
- a high proportion of unrelated vendored, copied, or generated material.

### 8.2 Originality and Differentiation — 0–100

Originality measures whether the project contributes a distinct solution,
implementation, integration, dataset, workflow, or insight.

Positive evidence includes:

- a clearly articulated problem and differentiator;
- original domain logic, interfaces, data, algorithms, or developer experience;
- meaningful and documented design tradeoffs;
- a novel combination that creates demonstrable new utility; and
- independent usage or discussion recognizing distinct value.

Similarity evidence includes:

- source- and AST-level similarity;
- README, documentation, and asset similarity;
- package metadata and public API similarity;
- repository template or fork lineage; and
- families of near-identical repositories.

Similarity is not proof of misconduct. Assay MUST identify forks, templates,
generated files, vendored files, and properly licensed reuse before evaluating
differentiation.

### 8.3 Engineering Rigor — 0–100

Engineering Rigor measures whether the project appears correct, maintainable,
secure, and reproducible for its type and maturity.

Signals include:

- coherent installation, build, execution, and packaging configuration;
- meaningful automated tests and successful reported CI execution;
- type checking, linting, static analysis, and dependency hygiene;
- error handling, configuration, observability, and safe defaults;
- complexity, duplication, dead code, placeholder, and generated-code ratios;
- documentation, examples, public interfaces, and code consistency;
- release artifact integrity and versioning; and
- durability, revert, hotfix, and defect evidence from Assay's core engine.

Security and dependency health are Engineering Rigor criteria rather than a
separate top-level score. Their applicability and weight vary by type. A proof
of concept is not held to the deployment controls of a public service, while a
library is evaluated for supported runtimes, dependency health, safe defaults,
and package integrity appropriate to its use.

OpenSSF Scorecard checks MAY provide security evidence, but its aggregate score
MUST NOT be copied as Assay's Engineering Rigor score.

### 8.4 Open Source Readiness — 0–100

Open Source Readiness measures whether other people can legally understand,
use, verify, and contribute to the project.

Signals include:

- a clear open-source license and dependency provenance;
- installation, quick-start, examples, and API documentation;
- contribution, governance, support, and security expectations;
- versioned releases, changelog, and migration guidance;
- reproducible evaluation or benchmark instructions;
- explicit scope, limitations, maturity, and compatibility; and
- disclosure of generated data, generated code, or third-party assets where
  relevant.

### 8.5 Maintenance Health — 0–100

Signals include:

- issue and pull-request response patterns;
- release recency and cadence appropriate to project type;
- roadmap or milestone follow-through;
- review and contribution practices;
- contributor diversity and maintainer concentration;
- deprecation, migration, and security-response behavior; and
- evidence that reported problems result in changes.

Slow cadence is not inherently negative. Stable libraries, specifications,
datasets, and finished tools require lifecycle-aware expectations.
Inactivity alone MUST NOT reduce the score. Unsupported runtimes, unmanaged
dependencies, unresolved security exposure, broken current compatibility, or
unanswered critical reports MAY reduce it.

### 8.6 Potential — 0–100, separate indicator

Potential estimates evidence of future improvement and adoption over a
declared horizon. It remains separate from current Assay Score.

Signals may include:

- improving release, quality, and issue-resolution trends;
- sustained external contributor or user growth;
- a differentiated solution to an active problem;
- roadmap execution and narrowing technical risk;
- useful integration surfaces and ecosystem timing; and
- improving documentation, packaging, and operational maturity.

Potential MUST declare its forecast horizon, confidence, assumptions, and
major counter-signals. It is not financial or investment advice.

## 9. Score Model

### 9.1 Published score card

```text
Assay Score                   0–100, provisional, or insufficient evidence
  Project Substance           0–100
  Originality                 0–100
  Engineering Rigor           0–100
  Open Source Readiness       0–100
  Maintenance Health          0–100

Potential                     0–100, separate
Confidence                    0.00–1.00 per score
Evaluation Version            explicit
Evidence Timestamp            explicit
```

### 9.2 Initial candidate weights

| Dimension | Weight |
| --- | ---: |
| Project Substance | 25% |
| Originality and Differentiation | 20% |
| Engineering Rigor | 25% |
| Open Source Readiness | 15% |
| Maintenance Health | 15% |

Potential is not included in Assay Score. Weights are provisional until
calibration and MUST be version-controlled. Type-specific applicability is
resolved before weighting. A young project MAY receive a provisional Assay
Score from sufficient applicable evidence even when long-term maintenance
evidence is unavailable. The result MUST disclose the provisional
normalization, low confidence, missing evidence, and criteria that could change
the score. Missing essential evidence still makes the overall score
unavailable under the versioned sufficiency policy.

### 9.3 Scoring invariants

- Deterministic checks and validated AI rubric judgments may contribute to
  published scores.
- An AI provider MUST NOT emit or override the final overall score directly.
- AI judgments contain criterion IDs, bounded ratings, confidence, cited
  evidence IDs, and concise rationale. The deterministic score compiler
  validates and weights those judgments.
- Each point contribution MUST be explainable by rule and evidence ID.
- Applicable and unavailable checks MUST be distinguished.
- Age, language, project type, maturity, and ecosystem cohorts MUST be declared.
- Equal scores across types have the common meaning that each project performs
  similarly well against its own applicable purpose and maturity rubric.
- Relative rank and similarity are reported only within a functional cohort.
- Stars, forks, watchers, download counts, and downstream usage MAY be shown as
  context but MUST NOT materially increase a score.
- Score changes MUST retain prior snapshots and explanations.
- Evaluation-version changes MUST trigger explicit rescoring.
- Maintainer annotations may add context but MUST NOT rewrite facts.
- Sponsorship, paid placement, or editorial featuring MUST NOT alter scores.

### 9.4 Automatic comparison contract

Assay automatically finds candidates from public GitHub evidence and stops at
one search depth. A discovered candidate MUST NOT seed another discovery pass.
The detail page shows five candidates with full comparison evidence and MAY
show additional candidates in a compact list.

Each comparison records why the candidate was selected, confidence, analyzed
revision, overall similarity, problem and feature overlap, technical and
structural similarity, and differentiating evidence. Similarity alone does not
reduce quality and MUST NOT imply misconduct.

### 9.5 AI evaluation contract

The evaluation engine builds a bounded `EvidenceBundle` from deterministic
collectors and analyzers. A versioned rubric asks the configured AI provider to
judge qualitative criteria that cannot be reduced safely to repository counts,
such as claim-to-implementation correspondence, differentiation, coherent
scope, and the credibility of a potential narrative.

The provider returns structured judgments only:

```json
{
  "rubric_version": "project-rubric-1",
  "judgments": [
    {
      "criterion_id": "substance.claim_implementation_fit",
      "rating": 3,
      "rating_scale": 4,
      "confidence": 0.82,
      "evidence_ids": ["evidence:readme:claim-4", "evidence:test:run-2"],
      "rationale": "The documented workflow is exercised by an integration test."
    }
  ]
}
```

The score compiler MUST reject unknown criterion IDs, out-of-range values,
missing citations, citations outside the supplied bundle, or schema-invalid
output. The evaluation snapshot records provider, model, prompt/rubric version,
sampling settings, usage, latency, and validation status.

Repository text is untrusted input. Prompts MUST delimit evidence from system
instructions, ignore instructions found inside repository content, restrict
tools, limit supplied source text, and validate structured output before use.

### 9.6 AI provider modes

| Provider mode | Intended use | Credential policy |
| --- | --- | --- |
| OpenAI API | Public web service and normal server deployments | Server-managed API key from secret storage |
| Codex CLI | Local CLI or a trusted single-operator installation | Reuse an existing local Codex login; never upload or copy its credential store |
| Codex OAuth | Experimental opt-in provider for the public site | Per-user authorization handled by an isolated OAuth broker and runner |

The Rust provider boundary is implemented in `assay-ai-evaluator`.

For the public multi-tenant service:

- OpenAI API key mode is the supported production default.
- API keys MUST remain server-side and MUST NOT be returned to the browser.
- The repository form MUST NOT ask users to paste an OpenAI key, Codex
  `auth.json`, ChatGPT cookie, OAuth refresh token, or Codex access token.

The optional Codex OAuth provider is an explicit product requirement despite
being an advanced and less portable deployment path. It MUST:

- use a browser redirect, device authorization, or other officially supported
  Codex login surface rather than asking the user to paste token material;
- run the Codex client or SDK inside an isolated per-user execution boundary;
- keep access and refresh tokens out of application logs, analytics, browser
  storage, URLs, job payloads, and repository-accessible environments;
- encrypt retained credentials with envelope encryption and a server-side KMS;
- isolate credentials by Assay user and never place them in a shared
  `CODEX_HOME`;
- support expiry, refresh, revocation, disconnect, and deletion;
- default to non-persistence when the supported login flow permits it;
- record security audit events without recording token values;
- fail closed when the official flow, required scope, or token audience cannot
  be verified; and
- remain behind a feature flag with provider-specific health checks and a
  server-managed kill switch.

Assay MUST NOT invent, reverse-engineer, or hard-code undocumented OAuth
endpoints, client credentials, scopes, or token exchange behavior. The adapter
is enabled only when a supported Codex authorization mechanism is available in
the deployed Codex CLI, SDK, or app-server version.

For local or trusted single-operator mode, the Codex adapter MAY invoke
`codex exec` using an already authenticated local installation. It MUST use a
read-only sandbox, ephemeral session where practical, a fixed output schema,
bounded time and resources, and no credentials supplied by repository content.

If AI evaluation is unavailable, Assay SHOULD return deterministic facts and
mark AI-dependent criteria and scores as `unavailable` rather than failing the
entire repository profile.

### 9.7 Hosted evaluation profiles

Credential and provider modes are separate from product evaluation profiles.
The initial deployment exposes stable `anonymous` and `authenticated` profile
IDs. Their concrete model, reasoning, sampling, candidate limits, and prompt
settings are deployment configuration rather than public schema constants.

Anonymous results publish automatically. Authenticated results begin as a
private preview and may be published explicitly. Profiles use the same score
dimensions and semantics but remain separate snapshots; one profile never
silently replaces or masquerades as the other. Every result records its profile
and runtime model metadata.

## 10. Functional Requirements

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| OPI-001 | P0 | Analyze a public GitHub repository from a URL or identifier. | The run records the canonical repository, immutable revision, collection time, and source availability. |
| OPI-002 | P0 | Classify project types, tags, and maturity. | Each classification contains a primary type, optional secondary types and tags, evidence, confidence, and `unknown` behavior. |
| OPI-003 | P0 | Calculate Substance, Engineering Rigor, and Open Source Readiness from deterministic evidence and validated AI rubric judgments. | Every score validates against the project schema and explains rule contributions. |
| OPI-004 | P0 | Represent missing and insufficient evidence explicitly. | Young projects may receive a labeled provisional score; unavailable criteria do not become misleading zeros. |
| OPI-005 | P0 | Generate an evidence-grounded project profile. | Every factual statement links to an evidence ID or is labeled as interpretation. |
| OPI-006 | P0 | Verify claim-to-implementation consistency. | Unsupported claims and broken examples are reported with source evidence, not author-intent language. |
| OPI-007 | P1 | Calculate originality using a versioned comparison corpus. | Forks, templates, licensed reuse, generated code, and vendored code are contextualized before similarity evaluation. |
| OPI-008 | P1 | Calculate Maintenance Health using lifecycle-aware rules. | Stable and completed projects are not penalized only for slow activity. |
| OPI-009 | P1 | Calculate a separate Potential indicator. | Horizon, confidence, assumptions, and counter-signals are published. |
| OPI-010 | P1 | Calculate a versioned overall Assay Score. | Dimensions, type-specific applicability, weights, provisional status, sufficiency, confidence, and evidence remain visible. |
| OPI-011 | P1 | Publish score history and explanations. | Users can identify changes caused by evidence, rules, or evaluation versions. |
| OPI-012 | P1 | Provide a contact path for factual or provenance concerns. | Users cannot edit or retry a score; administrators may hide, restore, or rerun an audited snapshot. |
| OPI-013 | P0 | Publish eligible anonymous results to a public catalog automatically. | Publication enforces public-source, license, safety, and evidence-sufficiency policy; authenticated previews remain private until explicitly published. |
| OPI-014 | P1 | Rescan listed projects after the profile cooldown. | Jobs are incremental, idempotent, repository-limited, and retain engine-specific snapshots. |
| OPI-015 | P1 | Discover and compare a one-depth functional cohort automatically. | Five detailed candidates show selection evidence, similarity, confidence, and differentiation without recursive discovery. |
| OPI-016 | P2 | Resolve package identifiers to source projects. | Resolution records registry evidence and refuses ambiguous matches. |
| OPI-017 | P0 | Analyze without executing repository code. | No dependency installation, import, build, test, or project script runs in hosted or local analysis. |
| OPI-018 | P1 | Evaluate awesome lists as curated artifacts. | Linked projects are not recursively analyzed; comparison is against similar curated lists. |
| AIE-001 | P0 | Provide a stable AI evaluator interface. | OpenAI API and Codex adapters consume the same evidence and return the same judgment schema. |
| AIE-002 | P0 | Support a server-managed OpenAI API key. | The key is loaded from secret storage, never exposed to repository code or the browser, and can be rotated without data migration. |
| AIE-003 | P1 | Support an existing local Codex CLI login. | Local Skill and CLI runs can use a read-only, bounded, structured Codex evaluation without copying the credential store. |
| AIE-004 | P1 | Support experimental Codex OAuth on the public site. | An isolated per-user broker implements supported login, encrypted retention, refresh, revocation, deletion, audit, and fail-closed behavior. |
| AIE-005 | P0 | Validate all AI judgments before scoring. | Unknown criteria, invalid ratings, and missing or fabricated evidence citations are rejected. |
| SKL-001 | P1 | Package Project Intelligence as an Agent Skill. | The skill invokes public CLI contracts and can analyze, explain, compare, and introduce a project without reimplementing scores. |
| WEB-001 | P0 | Accept a GitHub URL in the public web frontend. | A valid URL resolves to a canonical repository and an existing result, in-flight job, or newly admitted evaluation. |
| WEB-002 | P0 | Show asynchronous analysis progress. | The page shows elapsed time and the current named stage, then transitions to the result without a fabricated percentage. |
| WEB-003 | P0 | Publish engine-specific result pages and README badges. | Badges identify profile, score, provisional or stale state, and link to the matching result. |
| WEB-004 | P0 | Serve the shared dashboard from a local Assay instance. | Local private results render through the versioned API on a loopback-only server without source upload. |
| QUA-001 | P0 | Deduplicate equivalent evaluations. | The key includes repository ID, commit SHA, evidence version, evaluation version, rubric version, and canonical evaluator profile. |
| QUA-002 | P0 | Serve reusable results without quota charge. | Cache hits and joins to an in-flight equivalent job do not decrement daily evaluation quota. |
| QUA-003 | P0 | Apply a fourteen-day anonymous repository/profile refresh cooldown. | A duplicate anonymous submission navigates to the public result and cannot force a new run inside the cooldown. |
| QUA-004 | P0 | Apply a seven-day authenticated account/repository/profile refresh cooldown. | A member may create a private authenticated-profile snapshot only when the cooldown and revision policy admit it. |
| QUA-005 | P0 | Apply IP, account, repository, owner, failure, provider, and global limits separately. | Authentication, profile switching, or repeated repository requests cannot bypass safety controls. |
| QUA-006 | P0 | Reserve quota atomically. | Concurrent submissions cannot overspend a bucket; failed admission or infrastructure failure releases the reservation. |
| QUA-007 | P0 | Minimize IP data. | Rate limiting uses a daily keyed IP pseudonym, trusts forwarding headers only from configured proxies, and follows a documented retention period. |
| QUA-008 | P1 | Detect automated submission abuse. | Burst, failure, repository, owner, provider, and global circuit-breaker limits operate independently from daily user quota. |

## 11. CLI and Agent Interfaces

### 11.1 CLI commands

```text
assay project analyze <repository>
assay project explain <run-or-project>
assay project card <run-or-project>
assay project history <project>
assay project compare <project>... --cohort <cohort>
assay project submit <run>              # connected mode only
assay serve                             # loopback-only local dashboard
```

These commands inherit the main Assay non-interactive, output, schema,
logging, and exit-code contracts.

Local analysis accepts an evaluator selection such as
`--evaluator deterministic`, `--evaluator openai-api`, or
`--evaluator codex-cli`. The CLI MUST NOT accept raw OAuth tokens on the
command line because process arguments may be observable by other users.

An already cloned private repository needs no GitHub credential. A local
remote-private workflow MAY accept `--github-token-env <variable-name>` and
read the personal access token from that environment variable or an ignored
local `.env` file. It MUST NOT accept the token value as an argument, print it,
store it in an analysis result, or transmit it to the hosted service.

### 11.2 Machine envelope

```json
{
  "schema_version": "1.0.0",
  "evaluation_version": "project-intelligence-1",
  "evaluator": {
    "profile": "canonical-project-evaluator-1",
    "provider": "openai-api",
    "model": "recorded-at-runtime",
    "rubric_version": "project-rubric-1"
  },
  "project": {},
  "classification": {},
  "scores": {
    "assay_score": {},
    "project_substance": {},
    "originality": {},
    "engineering_rigor": {},
    "open_source_readiness": {},
    "maintenance_health": {},
    "potential": {}
  },
  "evidence": [],
  "introduction": {},
  "warnings": [],
  "limitations": []
}
```

### 11.3 Agent Skill behavior

The Agent Skill may answer:

- what the project does and who it is for;
- whether the implementation substantiates its claims;
- how original, robust, and open-source-ready it appears;
- what evidence supports its current value and future potential; and
- which missing evidence prevents a confident conclusion.

The agent MUST use validated Assay output, cite evidence, distinguish facts
from interpretations, and avoid author-intent or AI-usage claims.

## 12. Web Submission, Cache, and Quotas

### 12.1 Submission flow

The public page presents one primary GitHub URL field. Submission performs:

1. syntax and allowed-host validation;
2. canonical repository and current revision resolution;
3. authentication-state and evaluator-profile selection;
4. public, private, cached, and in-flight result lookup;
5. profile cooldown and abuse-limit admission;
6. atomic capacity reservation for an uncached run;
7. asynchronous job creation; and
8. redirect to a progress page that transitions to the result.

The browser never chooses arbitrary clone destinations or server fetch URLs.
The initial release accepts GitHub hosts only, preventing general URL fetching
and SSRF through the repository field.

### 12.2 Evaluation identity and caching

An equivalent evaluation is identified by:

```text
provider repository ID
+ immutable commit SHA
+ evidence extractor version
+ evaluation and scoring version
+ rubric version
+ canonical evaluator profile
```

Account identity is not part of the content evaluation key, although it scopes
private visibility and authenticated refresh admission. Provider executions
using the same evaluator profile SHOULD reuse equivalent judgments. If model,
reasoning, prompt, or candidate differences materially change evaluation, they
require a new evaluator profile and separate snapshot.

Unchanged results remain readable after a quota is exhausted. A user cannot
force refresh merely by resubmitting the same URL. Refresh admission occurs
only when the source revision, evaluator profile, evidence policy, or stale
snapshot policy requires it.

### 12.3 Refresh cooldown and abuse policy

The initial refresh policy is:

| Profile | Cooldown key | Minimum interval |
| --- | --- | ---: |
| Anonymous | repository and evaluator profile | 14 days |
| Authenticated | account, repository, and evaluator profile | 7 days |

If an anonymous public result already exists, another anonymous submission
navigates to that result and cannot create a duplicate run. An authenticated
member may request a separate private authenticated-profile result when its
cooldown admits the request. An unchanged revision normally returns cached
evidence instead of recomputing it.

Cooldown is not the abuse quota. Admission also applies independently
configured IP, account, repository, owner, failure, provider, and global
capacity limits. Cached results and joining an in-flight job are free.
Infrastructure or provider failure releases a capacity reservation. Repeated
failures trip a separate limiter.

### 12.4 IP and proxy handling

- Derive client IP only through the configured reverse-proxy trust chain.
- Ignore spoofable forwarding headers from untrusted peers.
- Store a daily rotating keyed HMAC of the normalized IP for quota lookup.
- Do not expose the pseudonym through public APIs or logs.
- Document retention, deletion, IPv4/IPv6 normalization, and UTC reset time.
- Apply coarser subnet, repository, account, owner, provider, and global
  limits only as necessary to control abuse and document their tradeoffs.

### 12.5 Frontend states

The Next.js interface provides:

- GitHub URL entry and canonical repository preview;
- profile, cooldown, and next eligible analysis time;
- immediate cached-result navigation;
- elapsed time and the current queued, collecting, classifying, comparing,
  evaluating, compiling, or publishing stage without a fabricated percentage;
- partial and provider-unavailable states without a user-controlled retry
  action;
- dimension score cards, confidence, evidence, and introduction; and
- a clear explanation that the evaluation concerns the project, not its
  authors or their use of development tools.

### 12.6 Local private dashboard

`assay serve` exposes the same versioned report contract and reusable frontend
components from a loopback-only local Rust API. A single-user local installation
does not require hosted membership. It retains immutable local history and
treats the local operator as its administrator.

Private-source AI evaluation and public-competitor discovery default to
`disabled` with reason `user_consent_required`. The page shape does not fork;
each section reports `complete`, `partial`, `pending`, `disabled`, or
`unavailable` plus a reason and allowed next action. Only explicit informed
consent may enable an external provider, and the result remains excluded from
the hosted catalog and public comparison corpus.

## 13. Public Introduction and Catalog

Each project page contains:

- project name, canonical links, license, type, and maturity;
- a one-sentence description and evidence-grounded introduction;
- target users, problem, differentiators, and primary use cases;
- a documented installation or demonstration path when evidence supports it;
- dimension scores, Assay Score, Potential, provisional state, and confidence;
- notable strengths, limitations, missing evidence, and review flags;
- activity, release, durability, and score history;
- source evidence and evaluation-version links;
- five detailed similar projects and optional compact additional candidates;
- engine-specific snapshots and README badge links; and
- a contact link for factual or provenance concerns.

LLMs MAY draft and update the introduction from the evidence package. The
publication pipeline MUST reject uncited factual claims, unsupported
comparisons, promotional superlatives, and statements that contradict
deterministic facts.

Catalog inclusion is separate from scoring. Editorially featured or sponsored
projects MUST be visibly labeled, and placement MUST NOT influence evaluation.
The initial home page introduces Hermes Agent and OpenClaw as independent
featured cards, followed by `Recently Assayed` and `Top Assays`. Detail pages,
not the home cards, present project comparisons. Reactions, comments,
bookmarks, follows, project claims, and formal appeals are deferred.

## 14. Server Architecture

`assay-project-intelligence` owns project profiling, score rules, comparison
corpus interfaces, and catalog-card domain objects. It consumes facts from the
existing Git, GitHub, classifier, semantic-diff, metrics, and storage crates.
`assay-ai-evaluator` owns rubric and provider contracts.

`apps/assay-codex-broker` is a separately deployable security boundary for the
experimental public Codex OAuth provider. The browser begins authorization
through the API, but the broker owns callback validation, encrypted token
storage, refresh, revocation, and isolated Codex execution. The API and general
worker exchange only an opaque provider-connection ID and validated judgment
payload; they never receive raw Codex tokens.

Hosted processing uses these job stages:

1. canonicalize source;
2. return an equivalent cached result or join its in-flight job;
3. atomically reserve the applicable quota;
4. collect metadata and immutable source;
5. classify project type and maturity;
6. run static, reported-CI, history, dependency, security, and documentation
   checks without executing repository code;
7. discover and compare a one-depth functional cohort;
8. call the configured AI provider with the bounded evidence bundle;
9. validate rubric judgments and compile scores;
10. draft and validate the project introduction;
11. store an immutable evaluation snapshot and consume the reservation; and
12. publish an anonymous result or retain an authenticated private preview.

Expensive similarity and AI jobs MUST be resource limited and cached.
Untrusted repository code MUST NOT execute in the API, worker, local analyzer,
or a separate build sandbox. Partial stage failure preserves completed results;
only an administrator may request an audited stage rerun after bounded system
retries are exhausted.

The hosted API SHOULD expose:

```text
POST /api/v1/project-evaluations
GET  /api/v1/project-evaluations/{id}
GET  /api/v1/projects/{provider}/{owner}/{repository}
GET  /api/v1/projects/{provider}/{owner}/{repository}/badge.svg
GET  /api/v1/catalog/recent
GET  /api/v1/catalog/top
GET  /api/v1/quota
POST /api/v1/admin/project-evaluations/{id}/rerun-stage
GET  /api/v1/providers/codex/oauth/start
GET  /api/v1/providers/codex/oauth/callback
DELETE /api/v1/providers/codex/oauth
```

OAuth route details are adapter-owned and are enabled only when supported by
the deployed Codex authorization mechanism. State, nonce, PKCE where supported,
redirect allowlists, CSRF defenses, and tenant binding are mandatory.

The initial PostgreSQL model includes project sources, immutable evaluation
snapshots, in-flight job leases, quota reservations and ledger entries,
engine-specific visibility, provider-connection metadata, and security audit
events. OAuth ciphertext and encryption metadata SHOULD live in a broker-owned
store or schema inaccessible to the normal API and worker roles.

## 15. Anti-gaming and Safety

- Prefer historical and cross-source evidence over badges or README claims.
- Prefer successful reported CI execution over the mere presence of test files.
- Verify that packages, releases, demos, and documentation correspond to the
  analyzed revision where possible.
- Detect copied boilerplate and claim mismatches without inferring intent.
- Rate-limit submissions and deduplicate forks, mirrors, and renamed projects.
- Provide a contact route for factual or provenance errors; users cannot edit
  or retry scores directly.
- Keep evaluator rules and versions public enough to explain results while
  testing resistance to superficial score optimization.
- Never install, import, build, test, or execute analyzed repository code.

## 16. Validation and Calibration

### 16.1 Benchmark groups

The versioned benchmark SHOULD include:

- mature projects across types and languages;
- promising young projects with limited popularity;
- honest templates, tutorials, examples, and proofs of concept;
- abandoned but historically valuable projects;
- thin wrappers and minimally differentiated clones;
- mass-published repository families with superficial differences;
- misleading documentation and non-working demonstrations; and
- projects with licensed, declared, and valuable reuse.

### 16.2 Human review

- Use at least two independent reviewers for calibration labels.
- Record reviewer rationale and disagreement.
- Measure inter-rater agreement by dimension.
- Evaluate false positives for young, small, educational, non-English,
  low-activity, and single-maintainer projects.
- Do not train and evaluate similarity models on overlapping project families.

### 16.3 Provider, quota, and cache tests

The service test suite MUST cover:

- OpenAI API success, schema failure, timeout, rate limit, and secret redaction;
- local Codex CLI success, missing login, expiry, timeout, and read-only sandbox;
- Codex OAuth state/nonce mismatch, callback replay, expiry, refresh, revoke,
  disconnect, tenant isolation, encryption, and kill-switch behavior;
- repository content attempting prompt injection or evidence-ID fabrication;
- simultaneous submissions against profile cooldown and capacity limits;
- anonymous and authenticated profile requests behind one IP;
- cached and in-flight duplicate requests that consume no quota;
- infrastructure failures that release reservations;
- spoofed forwarding headers and configured reverse-proxy chains; and
- repository, owner, failure, provider, and global circuit breakers.

### 16.4 Public score release gate

Before publishing Assay Score or Potential:

- dimension rules and weights are documented;
- evidence drill-down works for every rule;
- insufficient-data behavior is tested;
- age, ecosystem, type, maturity, and popularity bias are measured;
- superficial score-gaming attempts are tested;
- the evaluation version is frozen and reproducible; and
- a contact, administrative rerun, and version-migration process exists.

## 17. Delivery Plan

### Phase A — Repository profiler

- Submission, canonicalization, and evidence manifest.
- GitHub URL frontend, canonical cache key, and in-flight job deduplication.
- Anonymous and authenticated evaluator profiles with repository cooldowns.
- Project type, secondary type, tag, and maturity classification.
- Static, reported-CI, dependency, documentation, license, release, and
  maintenance checks without repository execution.
- CLI profile with no overall score.
- Loopback-only local dashboard and private GitHub source support.

### Phase B — Hybrid quality dimensions

- Project Substance, Engineering Rigor, and Open Source Readiness.
- OpenSSF Scorecard structured-evidence integration.
- Claim-to-implementation consistency checks.
- OpenAI API evaluator, structured rubric judgments, and score compiler.
- Provisional Assay Score, private preview, and public result UI.

### Phase C — Originality and comparison corpus

- Fork-, template-, generated-, and vendored-aware fingerprints.
- Source/AST, documentation, package, and asset similarity.
- Automatic one-depth functional cohort and versioned comparison corpus.
- Five detailed similar-project comparisons and compact additional candidates.
- Originality dimension with calibrated confidence.

### Phase D — Value, potential, and catalog

- Maintenance Health and calibrated Assay Score.
- Separate Potential indicator with an explicit horizon.
- Experimental Codex OAuth broker and isolated runner as an optional provider
  connection, independent from membership and evaluator-profile policy.
- Public catalog, score history, engine-specific badges, cooldown rescans,
  Contact handling, and editorial featuring.

### Phase E — Agent workflows

- Agent Skill commands for analyze, explain, compare, and introduce.
- Evidence-bounded summaries and manual-review escalation.
- Server integration for scheduled discovery and recommendations.

## 18. Open Decisions

1. Catalog safety, hiding, and Contact-triage policy after automatic anonymous
   publication.
2. Comparison-corpus construction, storage, and licensing.
3. Type, secondary-type, tag, and maturity classifier rules.
4. Large-repository streaming, cache, and hard safety limits.
5. Initial sufficiency thresholds and provisional-score confidence
   calibration.
6. Registry, dependency, citation, and adoption metadata providers.
7. Whether maintainers can request catalog hiding through Contact while
   preserving
   public analytical reproducibility.
8. Stale-score and badge policy beyond the fixed profile cooldowns.
9. Editorial review requirements before public introduction.
10. The supported Codex authorization surface and token lifecycle for each
    deployed Codex CLI, SDK, or app-server version.
11. Whether Codex OAuth connections are session-only by default or may be
    remembered with explicit consent.

## 19. Reference Implementation Inputs

- OpenSSF Scorecard provides structured, explainable heuristic checks for
  open-source security posture and documents the limitations of aggregate
  heuristic scoring: <https://github.com/ossf/scorecard>
- GitHub repository and metrics APIs provide structured public evidence for
  repositories, contributors, community profile, releases, and related state:
  <https://docs.github.com/en/rest/repos/repos>
- The Codex manual documents ChatGPT and API-key login, local credential
  caching, non-interactive execution, structured output, and the security
  sensitivity of saved authentication:
  <https://developers.openai.com/codex/codex-manual.md>

Assay uses these as evidence sources and implementation references, not as a
replacement for its own versioned project-substance and value model.
