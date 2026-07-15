# ADR 0001: Public JSON Schema Authority

- Status: Accepted
- Date: 2026-07-16

## Context

Assay needs stable public machine contracts before the CLI, evaluator
adapters, and score compiler are implemented. Core Rust values already model
some related concepts, but their internal evolution and release cadence must
not silently define the public schema version. Generating schemas from those
types now would also make incomplete implementation details look normative.

Maintaining both generated and hand-authored schemas without validating one
against the other would create two conflicting sources of truth.

## Decision

The reviewed Draft 2020-12 documents under `schemas/` are the authoritative
public contracts. They are hand-authored and versioned independently from
internal Rust types. Rust serializers and consumers must validate their output
against these schemas through reviewed golden contract tests.

All public object shapes are closed with `additionalProperties: false`.
Schemas use internal references only, so validation never resolves a network
resource. Major-version compatibility and unknown-field behavior are defined
in `schemas/README.md`.

`analysis-manifest/v1.json` and `project-evidence/v1.json` are complete for the
foundation evidence slice. `ai-judgment/v1.json` and
`project-evaluation/v1.json` are reviewable contract skeletons: they establish
citations, statuses, confidence, deterministic compilation, and the separation
of project dimensions and Potential without freezing unsettled scoring policy.

## Alternatives considered

### Generate JSON Schema from Rust types

This would reduce manual duplication, but the current Rust types do not yet
cover the complete public contracts. It would couple public schema versions to
internal refactoring and expose partial implementation choices as product
policy.

### Keep generated and hand-authored schemas

This would provide flexible documentation, but it creates two authorities
unless every semantic constraint is proven equivalent. The project does not
need that complexity for the foundation slice.

### Use permissive objects for forward compatibility

This would let older validators ignore new fields, but it would also allow
misspellings and undocumented interpretations. Assay instead chooses closed
objects and explicit version negotiation.

## Consequences

- Public schema changes require direct review and compatibility analysis.
- Rust serialization tests must validate actual instances, not generated
  approximations.
- Additive v1 changes remain backward compatible for instances, while older
  validators may need an updated v1 artifact to accept new optional fields.
- Breaking meaning, required-field, unit, or enum changes require a new major
  schema and migration note.
- The schemas cannot depend on runtime services or network resolution.
