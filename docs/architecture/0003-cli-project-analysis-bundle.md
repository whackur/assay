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

Schema validation is followed by deterministic cross-record checks for one
repository and revision, sorted unique evidence IDs, matching data-source
revisions, and the exact evidence artifact count, status, and SHA-256 digest.

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
- The CLI implements `project analyze`, not the future top-level alias.

## Rejected alternatives

- An unconstrained schema shell was rejected because empty nested objects
  would validate.
- Copying component definitions was rejected because they could drift.
- Hashing host paths was rejected as non-portable and privacy-sensitive.
- Hashing only `HEAD` was rejected because identity would change per revision.
