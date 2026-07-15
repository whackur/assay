# Schemas

The JSON Schema documents in this directory are Assay's authoritative public
machine contracts. They are reviewed, hand-authored Draft 2020-12 schemas.
Rust types may implement or consume these contracts, but Rust type versions
remain independent and do not generate a second public schema.

## Contracts

| Contract | v1 status | Purpose |
| --- | --- | --- |
| `analysis-manifest` | Complete | Immutable source, run status, provenance, scope, and artifact manifest |
| `project-evidence` | Complete | Citable, privacy-bounded project facts without raw source or person scores |
| `ai-judgment` | Reviewable skeleton | Bounded rubric ratings with required evidence citations |
| `project-evaluation` | Reviewable skeleton | Dimensioned project-score envelope compiled from cited evidence |

Every instance declares `schema_version`. The `v1.json` schemas accept only
`1.x.y` versions. A new major version requires a new schema artifact and an
explicit migration note.

## Unknown fields and compatibility

Every object is closed with `additionalProperties: false`. Producers must not
emit undocumented fields, and consumers must not silently interpret them.

Within major version 1, a later schema revision may add optional fields or
tighten only behavior that was already invalid. It must continue to accept
previously valid v1 instances, preserve every existing field and enum meaning,
and keep omitted optional fields semantically neutral. Renaming or removing a
field, making an optional field required, changing an enum meaning, changing a
unit, or changing unavailable data into a numeric zero requires a new major
version. Older validators are not promised forward compatibility with newly
documented optional fields; clients must negotiate `schema_version` and use a
matching bundled schema.

## Validation and references

Schemas use only internal `#/$defs/...` references. Their HTTPS `$id` values
are stable identifiers, not network dependencies. Contract tests compile each
schema after validating it against the Draft 2020-12 meta-schema, validate
every reviewed golden, and reject missing fields, unknown fields, unsupported
major versions, unknown statuses, uncited AI judgments, absolute source paths,
and person-level scores.

Run the validator with:

```sh
cargo test -p assay-cli --test schema_contracts
```

When Rust serialization implements a public contract, its serialized output
must be added as a reviewed golden and validated by this test. Do not compare a
Rust-generated schema with these files as an unverified second source of
truth.
