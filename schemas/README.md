# Schemas

The JSON Schema documents in this directory are Assay's authoritative public
machine contracts. They are reviewed, hand-authored Draft 2020-12 schemas.
Rust types may implement or consume these contracts, but Rust type versions
remain independent and do not generate a second public schema.

## Contracts

| Contract | v1 status | Purpose |
| --- | --- | --- |
| `analysis-manifest` | Complete | Immutable source, effective-config hash, component versions, status, scope, and artifacts |
| `project-evidence` | Complete | Citable facts or explicit availability envelopes without raw source or person scores |
| `project-analysis` | Complete | Offline composition of one manifest and its evidence records |
| `capabilities` | Complete | Exact implemented commands, formats, schemas, languages, and feature states |
| `ai-judgment` | Complete | Bounded rubric ratings with required evidence citations |
| `project-evaluation` | Reviewable skeleton | Dimensioned project-score envelope compiled from cited evidence |

`ai-judgment/v1.json` is the implemented provider-independent qualitative
judgment contract. `project-evaluation/v1.json` remains a reviewable contract
skeleton until deterministic score compilation is implemented.

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

The required provenance corrections recorded in ADR 0001 were made during
pre-release review, before any v1 contract was published. Once v1 is released,
the compatibility rules above apply without that exception.

## Status and provenance boundaries

An analysis manifest records the effective configuration hash and every
analyzer and parser component used. Component arrays are unique and producers
sort them lexicographically by `(name, version)` for deterministic output. The
parser array is present and empty when no parser was used.

`complete` and `partial` project evidence carries a factual payload, evidence
grade, and immutable provenance. `unavailable`, `unsupported`, `insufficient`,
and `pending` evidence carries no factual payload or provenance: it names only
the requested payload kind and a machine-readable reason within the common
repository, identity, status, and privacy envelope.

Tracked-file language values are present exactly when supported language
detection is complete. File-classification citations name exactly one raw
tracked-file record, partial attribute classification cites unavailable
attribute facts, and a missing classification never invents a policy attempt.
Parent-delta process and parse failures are availability envelopes rather than
factual payloads. Public path values are limited to 8,192 characters; producers
publish an explicit `path_length_limit` envelope instead of removing the bound
or disclosing a truncated path.

Repository-feature semantics are evaluated from the public evidence set. A
payload-free path-limit envelope cannot disclose whether it directly matches a
feature. In the absence of a reliable public match, every opaque tracked-file
envelope is therefore a global uncertainty cause for path-only features and
every opaque file-classification envelope is a global uncertainty cause for
classification-dependent features. The `related_evidence_ids` array is the
exact sorted cause set and participates in feature identity. A reliable match
takes precedence and a `present` feature cites only reliable matching facts.

This conservative rule states that the published evidence cannot establish
absence. It does not assert that an opaque record contains the feature and
does not assign likelihood, productivity, or project quality. Consumers must
not convert `unavailable` into `present`, `absent`, or a numeric zero.

Potential uses a contract distinct from Assay Score. It declares an ISO-8601
forecast duration plus cited assumptions and major counter-signals. Potential
is never folded into the current project score. The forecast duration must be
positive: zero durations are rejected, while the schema does not prescribe a
deployment-specific horizon. The format assertion remains responsible for the
duration grammar, so a non-zero digit alone cannot make an invalid duration
valid.

## Validation and references

Schemas use internal `#/$defs/...` references except that composition schemas
may use allowlisted same-origin bundled schema IDs. HTTPS `$id` values are
stable identifiers, not network dependencies. Validators register bundled
resources in memory and do not resolve HTTP. Contract tests compile each
schema after validating it against the Draft 2020-12 meta-schema, validate
every reviewed golden, and reject missing fields, unknown fields, unsupported
major versions, unknown statuses, uncited AI judgments, absolute source paths,
and person-level scores. Format assertions are mandatory: validators enable
Draft 2020-12 format validation so a string that merely resembles RFC 3339 or
ISO-8601 syntax cannot bypass semantic validation.

Tests discover all `schemas/*/v1.json`, matching goldens, and invalid fixtures;
reject duplicate JSON object keys; resolve every internal JSON Pointer
directly; and fail on an orphan fixture, dangling reference, or unregistered
external reference. Git null object IDs and path-like remote record IDs are
invalid.

Run the validator with:

```sh
cargo test -p assay-cli --test schema_contracts
```

When Rust serialization implements a public contract, its serialized output
must be added as a reviewed golden and validated by this test. Do not compare a
Rust-generated schema with these files as an unverified second source of
truth.
