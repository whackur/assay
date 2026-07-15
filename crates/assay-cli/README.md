# assay CLI

The implemented vertical slice is:

```text
assay project analyze [local-repository] \
  --revision <revision> \
  --evaluator deterministic \
  --format json \
  --output <path|-> \
  --no-color \
  --non-interactive

assay capabilities --format json --output - --no-color
```

The repository defaults to the current directory and the revision defaults to
`HEAD`. A movable revision is resolved once; collection uses the returned full
commit ID. JSON result data goes to stdout only for `--output -`. Diagnostics
go to stderr. File output uses atomic no-clobber persistence.

The command collects immutable local Git facts, applies the built-in v1 file
policy, and returns a schema-validated `project-analysis/v1` bundle. It does
not access the network, install dependencies, import, build, test, or execute
repository code. It calculates neither project scores nor person-level
observations. Local sources are treated as private because the CLI cannot
prove that a clone is public.

Repository paths remain content-bounded. A UTF-8 or hexadecimal path value
that exceeds the public 8,192-character limit is not serialized. The raw file
and its derived classification instead remain citable, payload-free
availability envelopes, and the manifest reports `path_length_limit` with a
partial result.

Feature states are derived only from the published evidence. Because a
payload-free envelope cannot reveal whether its hidden path directly matches a
feature, all opaque tracked-file envelopes conservatively cause uncertainty
for path-only features, and all opaque classification envelopes cause
uncertainty for classification-dependent features, when no reliable public
match exists. The resulting `unavailable` feature cites the exact sorted cause
IDs. If a reliable public match exists, `present` takes precedence and cites
only the matching facts.

This behavior reports evidence sufficiency, not the likely contents of a
hidden path. An opaque cause does not mean that the feature exists, and
`unavailable` must not be interpreted as `present`, `absent`, a project score,
or a quality judgment.

Attribute resolution, semantic diff, GitHub collection, AI evaluation, and
project scores are not implemented. `assay capabilities --format json`
reports these boundaries. Missing evidence remains an availability state and
is never replaced by zero.

Successful complete or explicitly partial output uses exit 0. Invalid
arguments use 2, missing sources or revisions use 4, collection failures use
10, analysis failures use 11, and output or schema failures use 12. Codes 3
(authentication or authorization) and 5 (retryable platform failure) are
reserved by the common contract but cannot occur in this local deterministic
slice. Diagnostics expose stable stages and categories, never paths, Git
output, tokens, or source text.
