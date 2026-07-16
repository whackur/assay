# ADR 0008: Place Classification and One-Depth Comparison Behind a Search Port

- Status: Accepted
- Date: 2026-07-16

## Context

Assay must classify a project's type, secondary types, tags, and maturity, and
must automatically discover a functionally similar cohort from public GitHub
evidence. The specification requires that classification carry evidence,
confidence, and explicit unknown behavior (OPI-002); that comparison discover a
one-depth cohort where a discovered candidate never seeds another search (9.4,
OPI-015); that popularity never raise a similarity or quality value; that an
awesome list be compared as a curated artifact against other curated lists
without analyzing its linked projects (OPI-018); and that unavailable or
insufficient comparisons stay explicit rather than becoming zeros.

Two placement questions followed. First, where the classification and
comparison logic live relative to the deterministic score compiler. Second, how
the comparison stage reaches GitHub without pulling network I/O into a crate
that must remain deterministic, and without letting a discovered candidate
re-enter discovery.

## Decision

### Classification and comparison live in `assay-project-intelligence`

Both stages are implemented in `assay-project-intelligence` alongside the
deterministic evidence manifest and score compiler, matching the architecture
boundary that keeps public project profiling, scoring, and similarity evidence
in that crate. Both perform no filesystem, process, network, clock, or provider
I/O, so identical input yields byte-identical output.

The classifier maps cited, evidence-grounded observations onto a
`ProjectClassification` — the exact value the score compiler already consumes —
so classification and scoring stay aligned without a second contract. A usable
classification requires both a resolved type and a resolved maturity because the
evaluation schema binds them; a type-only signal is represented as an explicit
unknown classification pending a maturity signal, never an invented default. A
separate versioned applicability policy resolves per-dimension
`RubricApplicability` from type and maturity, relaxing but never tightening
applicability for young or end-of-life projects so a young project is not
penalized for absent long-term evidence.

### GitHub search is a narrow injected port, and real wiring is deferred

Candidate discovery depends on a narrow `CandidateSearch` trait rather than on
GitHub collection. The trait is invoked once per analyzed project with a
`CohortQuery` that only a `SeedProject` can construct. A discovered `Candidate`
carries no profile and yields no query, so a candidate cannot re-enter
discovery: one-depth termination is a type-level guarantee, not a runtime check.
Deterministic fakes implement the port in tests; the real GitHub search adapter
is deferred and out of this change's scope.

### Comparison is a separate versioned artifact

The one-depth comparison is published as `schemas/project-comparison/v1.json`, a
new versioned contract separate from the evaluation envelope, because discovery
is a distinct job stage that runs before AI evaluation and produces its own
similarity evidence. Each mode has a closed canonical facet set —
`functional_cohort` uses the specification's problem overlap, feature overlap,
technical similarity, and structural similarity; `curated_list` uses the five
criteria of specification 7.3: entry overlap, list structure, unique coverage,
editorial quality, and maintenance evidence — and every detailed candidate
enumerates its mode's full set. Custom seed facets are rejected and
non-canonical candidate facets are ignored entirely, including in
differentiator output, so the contract stays enumerable and no undeclared
vocabulary reaches public output; the schema fixes the facet vocabulary as an
enum and requires the mode's exact facet set on every detailed candidate.
Similarity facets are computed with deterministic integer Jaccard arithmetic
over declared tokens; a facet without tokens on either side is an explicit
unavailable value, never a zero. A detailed candidate must earn at least one
cited selection reason: a candidate with no positive facet overlap is demoted
to an explicit `candidate_similarity_insufficient` limitation rather than shown
as a zero-similarity entry. Popularity is recorded as labeled context and used
only as an ordering tie-break. Curated-list mode compares an awesome list
against other curated lists and excludes non-curated candidates, and no linked
project is ever expanded because discovery never recurses.

## Alternatives

- Folding comparison into `schemas/project-evaluation/v1.json`. Rejected: the
  comparison stage runs and can fail independently of scoring, and coupling the
  two would force the compiler to own discovery.
- Deriving the full comparison profile from the current evidence bundle.
  Rejected: problem, target-user, and feature tokens require README and manifest
  semantics that a later collector or AI extraction supplies; the profile is a
  cited input here, matching how the compiler consumes pre-validated inputs.
- Enforcing one-depth discovery with a runtime recursion guard. Rejected: making
  a candidate structurally incapable of producing a query is a stronger and
  reviewable guarantee.

## Consequences

- Classification output feeds the existing compiler unchanged and validates
  against the evaluation schema.
- The comparison contract is stable and testable before any real GitHub search
  exists; wiring the adapter later does not change the contract.
- `ProjectType` gains a total order to support deterministic grouping; its
  serialized codes are unchanged.
- A future collector or AI extractor must supply comparison-profile tokens; the
  deterministic engine does not invent them.
