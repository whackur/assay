# ADR 0003: Compose the CLI Project Analysis Bundle Offline

- Status: Accepted
- Date: 2026-07-16

## Context

The first CLI slice must return one JSON result while preserving two existing
public meanings: `analysis-manifest` describes the immutable run and
`project-evidence` describes one citable fact or availability envelope. A
manifest alone cannot carry the evidence collection. The typed project
boundary also keeps raw tracked-file facts and derived classifications
separate; partial content, non-UTF-8 paths, gitlinks, and unavailable attribute
facts cannot be represented honestly by the legacy combined `file` payload.

## Decision

Add `project-analysis/v1` as a composition contract. Its nested fields
reference the same-origin, version-pinned HTTPS `$id` values for
`analysis-manifest/v1` and `project-evidence/v1`. Validators register bundled
schemas in memory and have no HTTP resolver. Unregistered references fail
closed.

Extend `project-evidence/v1` additively with `tracked_file`,
`file_classification`, and `parent_delta`. The legacy `file` meaning remains.
Raw and derived records cite one another but are not recombined. Partial facts
retain observed values and immutable provenance; unavailable facts remain
payload-free envelopes. Paths use validated UTF-8 or lowercase
`git_path_hex`.

Schema validation is followed by deterministic cross-record checks in
`assay-project-intelligence`. They close every citation over the evidence set;
match repository snapshot, history scope, data-source, provenance, and payload
facts; enforce status sufficiency; and verify the exact evidence artifact
count, status, and SHA-256 digest. The CLI only invokes this shared contract
validator and maps failure to its stable delivery error.

Public path values remain bounded to 8,192 characters. Longer UTF-8 or
hexadecimal paths are not serialized or truncated. Their raw and derived IDs
remain payload-free, citable availability envelopes, and the analysis is
explicitly partial with a `path_length_limit` limitation.

Repository-feature inference uses only the public evidence records. Once a
path is replaced by a payload-free `path_length_limit` envelope, a public
validator cannot determine whether the hidden path directly names a README,
license, package manifest, or a file in any classification category. When no
reliable public match exists, every opaque tracked-file envelope is therefore
a global uncertainty cause for each path-only feature, and every opaque
file-classification envelope is a global uncertainty cause for each
classification-dependent feature. Related IDs are sorted and included in the
feature identity. A reliable public match takes precedence: the feature is
`present` and cites only its reliable matching facts.

This policy measures whether the public evidence is sufficient to establish a
feature state. It cannot identify the hidden path, prove that every opaque
record contains the feature, or estimate a probability. `unavailable` means
that absence is not reviewable from the published evidence; it must not be
read as `present`, `absent`, or low project quality.

The Git adapter parses commit time and normalizes it to RFC 3339 UTC `Z`.
Local repository identity is a domain-separated SHA-256 over object format,
shallow state, and sorted reachable root commits. The identity operation also
returns the exact resolved commit; collection uses that full ID. Host paths
are never identity inputs. Shallow state is domain-separated because a shallow
boundary commit is not evidence of the complete repository root.

## Consequences

- Component definitions are not duplicated and validation never needs the
  network.
- Existing component instances remain valid; new producers use precise raw
  and derived payloads.
- Local input is `private_local` because the CLI cannot prove it is public.
- Attribute resolution is not implemented. Classification remains partial,
  and generated or vendored absence is unavailable rather than false.
- Opaque path envelopes conservatively widen feature uncertainty without
  disclosing or guessing the hidden path.
- The CLI implements `project analyze`, not the future top-level alias.

## Rejected alternatives

- An unconstrained schema shell was rejected because empty nested objects
  would validate.
- Copying component definitions was rejected because they could drift.
- Hashing host paths was rejected as non-portable and privacy-sensitive.
- Hashing only `HEAD` was rejected because identity would change per revision.
