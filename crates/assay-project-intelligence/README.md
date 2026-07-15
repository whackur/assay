# assay-project-intelligence

This crate assembles deterministic project-evidence facts from an immutable
`assay-git` snapshot and versioned `assay-classifier` decisions. It performs no
filesystem, process, network, database, GitHub, or model-provider I/O.

## What it measures

The initial contract records:

- the immutable repository source, commit, root tree, tracked paths, and Git
  object identifiers;
- bounded content metadata and explicit Git history and first-parent-delta
  availability;
- versioned file-policy category, tags, rule, confidence, and rule or resolved
  attribute provenance; and
- stable evidence identifiers for raw facts and derived classification facts.

Raw and classified facts remain in separate canonical collections. Missing,
partial, unavailable, and unsupported facts remain explicit. A missing file
classification is a citable unavailable record, not an absent file or a zero.

## What it cannot measure

This evidence does not establish that a project builds, runs, is correct,
secure, original, maintained, useful, or valuable. Repository code is not
installed, imported, built, tested, or executed. This crate does not calculate
project dimensions, an overall Assay Score, Potential, or any person-level
observation.

## Common misinterpretations

- A `generated`, `vendored`, `dependency`, or `unknown` classification is a
  file-policy description, not a quality penalty.
- Classification confidence describes rule evidence; it is not confidence in
  project quality.
- A content hash proves which bounded bytes were observed; it does not prove
  correctness or safety.
- Partial history or unsupported gitlink content is missing analytical
  evidence, not evidence of poor project quality.
- These facts must never be used to infer contributor effort, productivity,
  intent, compensation, or performance.

## Evidence identity and portability

Evidence IDs use a versioned, domain-separated, length-prefixed SHA-256
normalization. The scope includes the portable repository identity, immutable
revision and root tree, evidence kind, exact Git path bytes and object ID when
applicable, and canonical fact payload. Operational Git version provenance is
retained for explanation but is deliberately excluded from stable identity.

UTF-8 repository-relative paths use their validated portable spelling. Other
Git path bytes use lowercase hexadecimal encoding and their classification is
`unsupported`; the raw tracked-file fact remains citable. Local absolute paths,
source bytes, raw diffs, credentials, and person identities are not retained.

`ProjectEvidenceManifest` is currently a typed Rust boundary. It intentionally
does not implement a JSON serializer. The authoritative public machine
contracts remain the reviewed schemas under `schemas/`; CLI schema mapping and
golden output belong to the later CLI vertical-slice work.
