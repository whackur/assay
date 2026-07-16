# ADR 0007: Place the Score Compiler and the Judgment Contract Boundary

- Status: Accepted
- Date: 2026-07-16

## Context

The deterministic project score compiler combines two independent inputs into
dimensioned, confidence-aware scores: deterministic per-dimension rule
contributions, and validated qualitative rubric judgments. The specification
requires that a provider never emit or override a published score, that each
contribution be explainable by rule and evidence identifier, that
`not_applicable` and unavailable checks never become zeros, and that dimension
weights and the sufficiency rule be versioned data rather than scattered
constants.

The compiler consumes validated judgments produced by `assay-ai-evaluator`.
Placing the shared judgment contract wrongly would either create a dependency
cycle or pull heavy collection and classification code into the evaluator.
`assay-project-intelligence` already depends on `assay-classifier` and
`assay-git`; `assay-ai-evaluator` depends only on `assay-domain`.

## Decision

### The compiler lives in `assay-project-intelligence`

The compiler is implemented in `assay-project-intelligence` alongside the
deterministic evidence manifest, matching the architecture boundary that keeps
public project scoring in that crate. It performs no filesystem, process,
network, clock, or provider I/O, so identical input yields byte-identical
output.

### The shared judgment contract lives in `assay-domain`

The provider-independent judgment contract the compiler consumes —
`RubricApplicability`, `RubricCriterionId`, `RubricJudgment`, and
`RubricJudgmentSet` — lives in `assay-domain`. Both the compiler (consumer) and
`assay-ai-evaluator` (producer) already depend on `assay-domain`, so the
contract is shared without a new crate edge and without a cycle. The contract
is a validated data value with no provider, HTTP, or model concern, so it
belongs with the other core domain values.

`assay-ai-evaluator` maps its `ValidatedJudgmentSet` onto the domain contract
through `to_rubric_judgment_set`, which drops provider rationale and carries
only bounded ratings and citations. This proves the intended dependency
direction — `assay-ai-evaluator` targets the domain contract, never the reverse
— while keeping `assay-project-intelligence` free of any evaluator dependency.
A wiring crate such as `assay-cli` or the worker connects a concrete evaluator
to the compiler through this domain contract.

### The compiler input contract is owned by `assay-project-intelligence`

Deterministic contributions, the supplied classification, the potential
context, the evaluator descriptor, and the versioned `CompilerPolicy` are
compiler-local input types. Classification production, similarity, and
introduction generation remain separate stages; the compiler consumes a
resolved classification and cited potential context rather than producing them.

### Versioned scoring policy

Dimension weights (Substance 25, Originality 20, Rigor 25, Readiness 15,
Maintenance 15), the essential-dimension sufficiency rule, the partial-weight
factor, and confidence penalties are fields of a versioned `CompilerPolicy`.
All of them are folded, length-prefixed and domain-separated, into the
published `compiler.rule_set_hash`, so a policy change is visible in the
contract rather than a silent constant edit.

### Sufficiency and status semantics

The essential dimensions are Project Substance, Engineering Rigor, and Open
Source Readiness, matching the P0 scoring requirement. When an essential
dimension cannot be scored it is `insufficient`, and the overall Assay Score is
withheld as `insufficient` or `unavailable` with a null value. When every
essential dimension is scored but a non-essential dimension is missing, the
Assay Score is a `partial`, `provisional` normalization over the available
dimensions with reduced confidence and explicit missing-evidence limitations.
`not_applicable` contributions are excluded from weighting rather than scored as
zero. Potential is compiled with the same machinery but is never essential and
never enters the Assay Score.

## Consequences

- The provider surface cannot express a dimension or overall score; a provider
  influences a score only through a bounded, cited rubric rating.
- No dependency cycle exists, and the evaluator does not inherit collection or
  classification dependencies.
- Popularity signals (stars, forks, downloads) have no compiler input and
  therefore cannot raise a score.
- The reviewed evaluation golden is now reproduced by the compiler, so the
  scores contract is anchored to a real producer rather than a hand-authored
  fixture.
- Calibration values, the full deterministic rule catalog, similarity, and
  introduction generation remain out of scope and are supplied to or layered
  above the compiler later.

## Alternatives considered

- Placing the judgment contract in `assay-project-intelligence` and depending
  on it from `assay-ai-evaluator` was rejected because it would pull
  `assay-classifier` and `assay-git` into the evaluator for a data contract.
- Depending on `assay-ai-evaluator` from `assay-project-intelligence` was
  rejected because it reverses the intended direction and would let provider
  concerns leak toward the compiler.
- Emitting Potential narrative from the compiler was rejected because assumption
  and counter-signal prose is editorial; the compiler validates cited context
  and passes it through instead.
