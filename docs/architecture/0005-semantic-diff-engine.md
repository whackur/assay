# ADR 0005: Keep the Native Tree-sitter Adapter as the First Semantic-Diff Engine

- Status: Accepted
- Date: 2026-07-16
- Spike environment date: 2026-07-16

## Context

Assay must report added, removed, modified, moved, and renamed semantic units
for JavaScript, TypeScript, and Python sources, keep raw line facts separate
from semantic operations, and handle parse errors without fabricating
operations. The functional specification requires a spike that compares
native tree-sitter matching, difftastic-derived behavior, and GumTree on
identical reviewed fixtures before the first production engine is chosen.

`assay-semantic-diff` already provides a replaceable `SemanticDiffEngine`
trait and a native adapter (`native-tree-sitter-1`) over pinned tree-sitter
grammars. The reviewed fixture contract fixes the expected meaning of
format-only, body-modification, top-level-move, and symbol-rename changes for
each supported language. This record compares the three candidates and
selects the engine for the first production slice. It is a boundary decision,
not a permanent claim that one engine is best for every future language or
repository size.

## Decision drivers

1. Correct classification on the reviewed fixture contract;
2. explicit move and rename categories rather than delete-plus-add;
3. format-only changes producing no semantic operations;
4. explicit parse-error behavior that never invents operations;
5. cold and warm latency and peak memory on a reproducible baseline;
6. licensing and distribution consequences for a Rust CLI; and
7. integration complexity behind the existing engine trait.

## Candidate facts from primary sources

Checked on 2026-07-16 against the official repositories, manual, and release
pages listed under Primary sources.

| Fact | Native tree-sitter adapter | difftastic | GumTree |
| --- | --- | --- | --- |
| Version compared | `semantic-unit-matcher-1` on tree-sitter 0.26.11 | 0.69.0 (released 2026-04-30) | 4.0.0-beta4 binary; latest release is 4.0.0-beta8 (2026-07-15) |
| License | Assay code over MIT tree-sitter crates | MIT, with vendored parsers under MIT and Apache-2.0 | LGPL-3.0 |
| Implementation | Rust, in-process | Rust CLI binary | Java, requires JDK or JRE 17+ |
| Distribution | Part of the Assay workspace | crates.io source crate that builds the `difft` binary | GitHub release zip for beta4; beta5 through beta8 publish source archives only |
| Machine-readable output | Typed Rust values | None documented; terminal display plus `--exit-code` | Text, XML, and JSON edit-script dumps of match and action lists |
| Embedding in Rust | Direct library | No documented stable library API; child process required | Child JVM process required |

## Spike method

### Environment

The spike ran on an otherwise idle Linux container:

```text
Intel Celeron J4125 at 2.00 GHz, 4 cores, 4 MiB L2, x86_64
Debian GNU/Linux 13
rustc 1.97.0, release profile
difftastic 0.69.0 built from crates.io with cargo install
GumTree 4.0.0-beta4 release zip on Temurin JRE 17.0.19+10
```

External engine binaries and the JRE were installed only into a temporary
directory outside the repository and are not part of the Assay dependency
graph. GumTree 4.0.0-beta8 has no published binary distribution; building it
from source with Gradle was out of spike scope, so beta8 behavior is
explicitly not measured.

### Inputs and operations

All engines analyzed the same twelve reviewed fixture pairs committed under
the `assay-semantic-diff` test fixtures: for each of JavaScript, TypeScript,
and Python, the `before` source against the `format`, `modified`, `moved`,
and `renamed` variants. Each fixture file contains three small top-level
functions. No fixture content was installed, imported, built, tested, or
executed; engines only parsed the bytes.

- The native adapter was exercised in-process through the committed
  `semantic_diff_spike` example with 200 warm samples.
- difftastic ran once per pair as `difft --color never --display inline`
  (plus `--exit-code` for change detection), 20 warm bundle samples.
- GumTree ran once per pair as `gumtree textdiff`, 10 warm bundle samples.

A bundle is one pass over all twelve pairs. Cold is the first bundle in a
fresh process (native) or the first bundle of child invocations after
installation (external engines), so external cold values include first
page-in of large executables. Warm values are the median and nearest-rank
p95 of the remaining bundles. Peak RSS is `VmHWM` for the in-process native
run and the maximum child `ru_maxrss` for external engines. External
per-bundle times include twelve process starts (twelve JVM starts for
GumTree), while the native engine is in-process; the comparison measures each
engine at its realistic integration boundary, not isolated parser throughput.

To repeat the spike: install difftastic 0.69.0 with `cargo install` and the
GumTree 4.0.0-beta4 release zip with a Temurin JRE 17 into a temporary
directory, run the committed native example with `--samples 200`, and drive
both external binaries over the same fixture pairs with a timing harness that
records per-bundle wall time and child peak RSS. Machine-specific absolute
paths are deliberately excluded from this record.

## Results

### Correctness on the reviewed fixtures

| Variant | Native adapter | difftastic | GumTree |
| --- | --- | --- | --- |
| `format` (Python) | No operations | Exit 0, no change reported | Empty action list |
| `format` (JavaScript, TypeScript) | No operations | Exit 1; reports the deleted statement terminators | Three `delete-node` actions on the deleted terminators |
| `modified` | One `Modified` on the changed function | Change reported; no unit category | One `update-node` on the changed literal |
| `moved` | One `Moved` with the unit name | Change reported as two display hunks; no move category | One `move-tree` on the moved declaration |
| `renamed` | One `Renamed` with old and new names | Change reported; no rename category | One `update-node` replacing the declared identifier |

All three engines agreed wherever their output models could express the
expected meaning. The JavaScript and TypeScript `format` fixtures delete
statement terminators, so difftastic and GumTree correctly report token-level
deletions under their own semantics; the Assay contract intentionally treats
punctuation-only differences as format-only, which only the native adapter
expresses directly. GumTree's edit script localizes every change precisely,
including an explicit move action, but mapping `update-node` on a declared
identifier to Assay's `Renamed` category (rather than a generic edit) would
require Assay-side postprocessing. difftastic reliably distinguishes changed
from unchanged syntax but provides no unit-level move, rename, or
modification categories.

### Parse-error behavior

Given a valid TypeScript input against a syntactically broken one:

- the native adapter returned explicit parse-error facts and no operations;
- difftastic explicitly fell back to text-level diffing and labeled the
  output with the parse-error reason; and
- GumTree emitted seven edit actions with no error indication, silently
  diffing the parser's error-recovery tree.

The specification requires parse errors to produce a declared text-level
fallback and a partial run, never silent success. GumTree's silent
error-recovery actions would need Assay-side error detection before its
output could be trusted.

### Latency and peak memory

Times are microseconds for one twelve-pair bundle on the environment above.

| Engine | Samples | Cold | Warm median | Warm p95 | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native adapter (in-process) | 200 | 3,125 | 2,778 | 13,722 | 4,388 KiB |
| difftastic (12 processes) | 20 | 2,347,280 | 1,385,311 | 2,237,177 | 12,512 KiB |
| GumTree (12 JVM processes) | 10 | 28,691,979 | 7,685,069 | 26,153,435 | 86,588 KiB |

The native adapter is roughly 500 times faster than difftastic and 2,700
times faster than GumTree at this boundary, at a fraction of the memory. The
external numbers are dominated by process and JVM startup, which is exactly
the cost Assay would pay per analyzed file pair without a long-lived service
wrapper. These numbers are the reproducible baseline the specification
requires before numeric performance budgets are set; they are not a claim
about either tool's parser throughput on large files.

### Licensing, distribution, and integration cost

- The native adapter adds no new dependency beyond the already pinned MIT
  tree-sitter crates and stays compatible with a future self-contained CLI
  binary.
- difftastic is MIT and Rust, but it documents no stable library API, so
  integration means shipping or requiring a large external binary (the
  locally built stripped release binary is about 119 MiB due to dozens of
  vendored grammars) and parsing terminal-oriented output that offers no
  documented machine-readable mode.
- GumTree is LGPL-3.0 and requires a Java 17 runtime. Its current 4.0.0
  line publishes no binary distribution after beta4, so Assay would have to
  build it from source or pin a beta. The JVM requirement conflicts with the
  self-contained CLI goal, and LGPL-3.0 would add distribution obligations
  that MIT-licensed alternatives avoid.

## Decision

Keep the native tree-sitter adapter (`native-tree-sitter-1`) as the first
production semantic-diff engine. Do not integrate difftastic or GumTree into
production. Keep all engine behavior behind the `SemanticDiffEngine` trait so
a future engine can replace or accompany the native adapter without changing
domain inputs or public schemas.

The decision is based on:

- direct expression of the required unit categories, including moves and
  renames, which neither external candidate provides without postprocessing;
- exact agreement with the reviewed fixture contract, including
  punctuation-insensitive format-only detection;
- explicit parse-error facts instead of silent error-recovery output;
- in-process latency and memory that no child-process candidate approached;
- MIT-compatible licensing and no new runtime prerequisite; and
- zero additional integration surface in the Rust CLI.

GumTree remains the strongest reference for edit-script quality; its
`move-tree` and identifier-update actions matched the fixture meaning
precisely. It is the candidate to revisit first if Assay's matcher proves
insufficient on real-world corpora.

## Alternatives considered

### Integrate GumTree now

Rejected for the first slice: JVM runtime requirement, LGPL-3.0 distribution
obligations, no maintained binary distribution of the current release line,
roughly 640 ms per file pair at the process boundary, and silent
error-recovery output that would require Assay-side parse validation anyway.

### Derive behavior from difftastic

Rejected: difftastic is a display-oriented CLI without a documented library
API or machine-readable output, and its token-level model has no unit
matching from which move or rename categories could be read. Vendoring its
internals would mean maintaining a fork of a large codebase for behavior the
native adapter already provides.

### Postpone the choice

Rejected: downstream change-set and churn work needs a stable engine
identifier and operation contract now. The trait boundary keeps the decision
reversible.

## Consequences

- Production semantic diff stays in-process, deterministic, and dependency-
  light; analyzer provenance records `native-tree-sitter-1`, the pinned
  tree-sitter version, and the Assay rule version.
- Assay owns its matcher rules. The current matcher handles uniquely named
  top-level functions; duplicate-name matching, nested units, classes, TSX,
  and copy detection remain future rule versions behind the same trait.
- The reviewed fixture contract remains the regression gate for any rule or
  engine change.
- No LGPL or JVM obligation enters the dependency or release process.

Reopen this decision when any of the following is true:

- real-world corpora show the native matcher misclassifying moves, renames,
  or modifications at a rate that postprocessed GumTree output would beat;
- a required language cannot be served by a pinned tree-sitter grammar;
- GumTree's current line regains a maintained, license-compatible embedding
  path whose accuracy gains justify a service-style integration; or
- unit categories beyond the current contract (for example copies) prove
  infeasible under Assay's own matcher rules.

## Not measured

- GumTree 4.0.0-beta8 behavior (no binary distribution; source build out of
  spike scope).
- Copy detection and TSX fixtures (not part of the reviewed fixture
  contract yet).
- Large-file and repository-scale throughput; the fixtures are small
  single-file pairs, so these results baseline integration overhead, not
  parser scaling.

## Primary sources

The following sources were checked on 2026-07-16.

- difftastic repository, license statement, and vendored parser licensing:
  <https://github.com/Wilfred/difftastic>
- difftastic 0.69.0 release: <https://github.com/Wilfred/difftastic/releases>
- difftastic manual, syntactic diffing scope and exit codes:
  <https://difftastic.wilfred.me.uk/>
- GumTree repository and LGPL-3.0 license:
  <https://github.com/GumTreeDiff/gumtree>
- GumTree releases, 4.0.0-beta8 source-only assets, 4.0.0-beta4 binary zip,
  and the JDK 17 requirement:
  <https://github.com/GumTreeDiff/gumtree/releases>
- Eclipse Temurin 17 JRE binaries: <https://adoptium.net/>
