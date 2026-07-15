# ADR 0002: Use the Installed Git CLI Behind a Read-Only Adapter

- Status: Accepted
- Date: 2026-07-16
- Spike environment date: 2026-07-16

## Context

Assay needs to resolve an immutable commit, enumerate its tree, read history,
and detect renames without executing repository code. The adapter must retain
path bytes that are not valid UTF-8, avoid source text in diagnostics, and
remain replaceable as distribution requirements evolve.

The foundation milestone considered three implementations:

- the installed Git CLI;
- `gix`, a Rust implementation of Git; and
- `git2`, the Rust wrapper around `libgit2`.

This is a boundary decision, not a permanent claim that one implementation is
best for every Assay deployment.

## Decision drivers

The initial adapter needs:

1. Git-compatible revision and tree resolution;
2. rename and history behavior that can support later change-set and
   durability analysis;
3. lossless path bytes on platforms that support them;
4. acceptable latency on fixed small and medium repositories;
5. a small initial build and distribution burden;
6. a narrow attack surface for untrusted repositories; and
7. deterministic tests behind an `assay-git` port.

## Spike method

### Versions and machine

The spike used the following local toolchain on an otherwise idle Linux
container:

```text
Git 2.47.3, x86_64, libcurl 8.14.1, zlib 1.3.1
gix 0.85.0, Rust requirement 1.85
git2 0.21.0, libgit2-sys 0.18.5 with vendored libgit2 1.9.4
rustc 1.97.0
release profile
Intel Celeron J4125 at 2.00 GHz, 4 cores, 4 MiB L2
```

The installed Git version was obtained from `git version --build-options`.
Candidate metadata came from `cargo info gix@0.85.0` and
`cargo info git2@0.21.0`. The crates were pinned in a temporary Cargo project
outside the Assay workspace dependency graph. `gix` used no default features
and enabled only `sha1`, `revision`, and `blob-diff`; `git2` used no optional
network features.

### Repositories

The spike reused the deterministic builder in
`tests/support/assay-fixtures` and extended temporary repositories without
executing any tracked file:

| Set | Final tracked files | Commits | Distinguishing case |
| --- | ---: | ---: | --- |
| Small | 1 | 2 | Exact rename and move |
| Medium | 605 | 23 | 600 generated synthetic files, 20 revisions, final rename |
| Unusual paths | 3 | 2 | Spaces, Unicode, and `raw/invalid-<0xff>.ts` |

The medium extension used deterministic names `module-0000.ts` through
`module-0599.ts`. Initial file `N` contained
`export const valueNNNN = N;` followed by LF. Update `N` from 0 through 19
changed that value to `600 + N`, and the final commit renamed
`module-0599.ts` to `renamed-module.ts` without changing its bytes. Extension
commit timestamps were `2001-02-05T06:07:08+09:00`, the 20 dates from
`2001-03-01T06:07:08+09:00` through `2001-03-20T06:07:08+09:00`, and
`2001-04-01T06:07:08+09:00`.

Temporary extension commits used `Assay Spike Author` and
`spike-author@example.invalid` as author, and `Assay Spike Committer` and
`spike-committer@example.invalid` as committer. Messages were
`Add medium synthetic tree`, `Update medium module NN`, and
`Rename medium module`. The unusual-path extension used the same identities,
timestamp `2001-02-05T06:07:08+09:00`, message `Add raw byte path`, and
synthetic bytes `export const rawPath = true;` followed by LF.

The invalid path was compared as bytes; its complete hex encoding was
`7261772f696e76616c69642dff2e7473`.

### Comparable operation bundle

Each adapter performed one complete scan per sample:

1. resolve `HEAD` to a commit ID;
2. resolve the commit's root tree ID;
3. recursively collect non-tree entries as raw path bytes;
4. walk and count reachable commits; and
5. compare `HEAD^` with `HEAD` using 50% rename detection.

The CLI implementation used argument arrays and NUL-delimited output. Its
five subprocesses used `rev-parse`, `ls-tree`, `rev-list`, and `diff-tree`
with `--no-ext-diff` and `--no-textconv`. The `gix` implementation opened the
repository with `gix::open::Options::isolated()`. The `git2` implementation
used `TreeEntry::name_bytes()` and manual recursion because its convenience
`Tree::walk()` callback represents the directory prefix as `&str`.

The harness performed one unmeasured warm-up, then 30 small or 20 medium
samples. It sorted elapsed times and reported the median and nearest-rank p95.
The OS object and filesystem caches were warm. This is adapter-boundary
latency, not a claim about isolated parser throughput: the CLI numbers include
five process starts while the library numbers are in-process.

The temporary comparison source and its private Cargo lockfile were deleted
after the run. To repeat the spike, create an isolated temporary crate with
the exact versions and feature sets above, build the three repositories from
the committed fixture builder using the declared deterministic extension,
perform the five operations in this section, and run:

```sh
cargo run --release --manifest-path <temporary-spike>/Cargo.toml
cargo tree --manifest-path <temporary-spike>/Cargo.toml \
  -p gix --prefix none -e normal
cargo tree --manifest-path <temporary-spike>/Cargo.toml \
  -p git2 --prefix none -e normal
```

The dependency counts below normalize repeated Cargo tree markers with:

```sh
sed 's/ (\*)$//' | sort -u | wc -l
```

Machine-specific absolute paths are deliberately excluded from the record.

## Results

### Correctness

All candidates returned the same commit ID, tree ID, non-tree path byte set,
reachable commit count, and rename count.

```text
small:
  commit=f270801d0690dae8fb4e66c21e38bfdc1c1a63a5
  tree=4ff5204fc21cd41562153fe538806e693ccd2c19
  files=1 commits=2 renames=1 all_equal=true
medium:
  commit=c8c5a28c9571574890f97688fe27f69eeeb25e78
  tree=62abd96b0c380546bf15750f8a0e358f805f8e3d
  files=605 commits=23 renames=1 all_equal=true
unusual paths:
  commit=f4f53973b4d21a19ef01d7c9d8fb5effb98be471
  tree=2921471428d9b54391f45e62d86d4b3d7134b413
  files=3 commits=2 renames=0 all_equal=true
raw path 7261772f696e76616c69642dff2e7473:
  git-cli=true gix=true git2=true
```

The result demonstrates the tested cases only. It does not establish complete
behavioral equivalence across every revision expression, object format,
partial clone, replace reference, merge, or corrupted repository.

### Warm adapter latency

Times are microseconds for the complete operation bundle.

| Repository | Candidate | Samples | Median | p95 |
| --- | --- | ---: | ---: | ---: |
| Small | Git CLI | 30 | 10,946 | 12,613 |
| Small | `gix` | 30 | 1,005 | 1,345 |
| Small | `git2` | 30 | 789 | 811 |
| Medium | Git CLI | 20 | 11,954 | 17,929 |
| Medium | `gix` | 20 | 2,828 | 2,999 |
| Medium | `git2` | 20 | 3,428 | 3,660 |

The Git CLI is slower because the measured boundary starts five processes,
but its medium median remained about 12 milliseconds. The foundation
specification has not set a numeric Git collection budget. This overhead is
acceptable for the first deterministic slice and can later be reduced with
`cat-file --batch`, fewer commands, and incremental caching.

### Build, distribution, and implementation cost

| Candidate | Initial cost | Relevant consequence |
| --- | --- | --- |
| Git CLI | No new Cargo dependency | Requires a compatible, patched Git executable on the host and records its version in provenance. Git is GPL-2.0; bundling it would require a separate distribution review. |
| `gix` | 114 unique normal packages in the tested reduced feature graph | Pure Rust and no external Git process, but the current feature and API surface is large for the foundation slice. The documented isolated-open and trust model are valuable for a future embedded adapter. |
| `git2` | 6 unique normal packages in the tested local-only graph | Builds or links `libgit2` and zlib through native FFI. Cross-compilation and native vulnerability updates become Assay release concerns. Network features would add further native TLS and SSH choices. |

The first combined release build of both library candidates took 2 minutes
20 seconds and produced 233 MiB of release build artifacts on the spike
machine. That combined figure is not a fair candidate-by-candidate benchmark,
so it is recorded only as build-context evidence.

`git2` remained a technically viable candidate. Its tested local feature set
was compact and fast. It was not selected because its native C boundary and
cross-platform release burden provide no necessary capability over the
installed CLI for the first slice. The `gix` documentation also notes that
`git2` performs strict hash verification that `gix` does not yet provide in
the same way; this makes it inappropriate to describe `gix` as a strict
correctness superset.

### Security and testability

All three implementations parse attacker-controlled repository structures
and therefore require input size, time, recursion, and output limits.

- A CLI child process provides a failure boundary around Git parsing, but Git
  configuration, environment variables, external diff or text conversion,
  pathspecs, revision options, and executable selection must be constrained.
- `gix` avoids the shell and uses Rust for its parser. Its documented trust
  model and isolated open options reduce configuration exposure, but parsing
  still occurs inside the Assay process.
- `git2` also avoids the shell and provides typed APIs, but untrusted input is
  parsed by `libgit2` C code in the Assay process. Its Rust wrapper is safe to
  call; the native parser remains part of the deployed attack surface.
- The library APIs are easier to fake at individual method granularity. The
  CLI is still testable through a narrow process runner, exact byte protocol
  parsers, and the committed real-Git fixtures.

No candidate executed a tracked file during the spike.

## Decision

Use the installed Git CLI for the first production `assay-git` adapter. Keep
all Git behavior behind a domain-facing port so a `gix` or other embedded
adapter can replace it without changing domain inputs or public schemas.

The decision is based on:

- identical results in every reviewed spike case;
- Git's canonical revision, history, and rename behavior;
- lossless NUL-delimited path output;
- no new Rust or native library dependency;
- acceptable first-slice latency; and
- straightforward black-box integration testing with FIX-001 repositories.

Performance alone does not choose the adapter. Both embedded candidates were
faster in the measured bundle.

## Required CLI adapter boundary

The implementation of GIT-001 must enforce all of the following:

1. Resolve the Git executable once from trusted deployment configuration or a
   trusted startup environment. Never derive it from repository content.
2. Invoke `std::process::Command` directly with separate arguments. Never
   construct a shell command or interpolate user input into shell syntax.
3. Pass `--end-of-options` before an untrusted revision and peel it explicitly
   to `^{commit}`. Use full commit and tree IDs after resolution.
4. Use byte-oriented, NUL-delimited plumbing output such as `ls-tree -rz` and
   `diff-tree --raw -z`. Never require repository paths to be UTF-8.
5. Disable external diff and text conversion. Do not check out submodules,
   run hooks, apply working-tree filters, invoke credential helpers, or make
   network requests.
6. Disable replacement objects and optional locks for immutable analysis.
   Make rename thresholds and limits explicit instead of inheriting them from
   repository configuration.
7. Start from a minimal environment. Remove Git repository redirection,
   object-directory, index, work-tree, config-injection, pager, prompt, and
   tracing variables. Disable system and global configuration and override
   every repository-local setting that can affect the selected read-only
   operations.
8. Bound child lifetime, stdout, stderr, object size, record count, history
   depth, and rename candidate work. A limit produces an explicit partial or
   unavailable fact rather than a fabricated empty result.
9. Parse only documented machine formats and exit status. Do not expose raw
   stderr, source bytes, raw diffs, credential values, or machine-specific
   absolute paths in errors.
10. Probe required Git capabilities, record the exact Git version in analyzer
    provenance, and return a stable unavailable error if Git is absent or
    incompatible.
11. Keep source-content access read-only and object-based, preferably through
    a bounded `cat-file --batch` process. Never read a tracked executable in a
    way that launches it.

The production adapter must test hostile environment variables, a revision
beginning with `-`, spaces, Unicode, invalid UTF-8 path bytes on Unix, symlinks,
submodules, binary blobs, malformed output, timeouts, oversized output, missing
Git, and a non-zero child exit.

## Alternatives considered

### Use `gix` now

`gix` produced correct spike results, retained raw path bytes, supplied rename
tracking, and performed well. Its pure-Rust deployment and isolated trust
model make it the preferred candidate to revisit when Assay requires a
self-contained executable or process startup becomes material.

It was not selected now because the reduced spike still introduced a broad
dependency graph and more integration surface than the first slice needs.
The project would also need explicit corruption and compatibility tests around
the documented integrity-check differences.

### Use `git2` and `libgit2`

`git2` produced correct results and the lowest small-repository latency. It
has mature typed revision, tree, history, and diff APIs. Manual byte-oriented
tree recursion avoided the UTF-8 directory-prefix limitation of its
convenience tree walk.

It was not selected because the native parser, vendored-or-system linking
choice, C build toolchain, and cross-platform packaging cost are unnecessary
for the local-only first slice. If an embedded adapter becomes necessary,
`gix` should be evaluated first because it avoids the native boundary.

### Implement Git object parsing in Assay

This would minimize external behavior but duplicate complex Git semantics and
create an unacceptable correctness and security burden. It is rejected.

## Consequences

- Local analysis requires a compatible Git executable and must report its
  absence explicitly.
- Analyzer provenance includes the Git version and adapter identifier.
- Assay owns a small, security-sensitive byte protocol parser and process
  runner in `assay-git`.
- The process boundary adds measurable startup latency but isolates parser
  failures from the main process.
- The public schema and domain ports must not expose CLI-specific concepts.
- Distribution planning must either declare Git as a prerequisite or revisit
  an embedded adapter before claiming a self-contained binary.

Reopen this decision when any of the following is true:

- self-contained Linux, macOS, or Windows binaries become a release gate;
- compatible Git availability is unreliable on a supported target;
- measured Git process overhead violates a published collection budget;
- a required history or partial-clone behavior cannot be expressed safely
  through documented plumbing commands; or
- `gix` compatibility, integrity behavior, and a smaller selected feature
  graph are validated against the expanded Assay fixture corpus.

## Primary sources

The following sources were checked on 2026-07-16. Versioned URLs are used
where the publisher provides them.

- Git 2.47.3 revision verification and `--end-of-options`:
  <https://git-scm.com/docs/git-rev-parse/2.47.3/>
- Git 2.47.3 tree enumeration and verbatim NUL-delimited paths:
  <https://git-scm.com/docs/git-ls-tree/2.47.3/>
- Git 2.47.3 raw diff, rename detection, and `-z` behavior:
  <https://git-scm.com/docs/git-diff/2.47.3/>
- Git history and `--follow` limitations:
  <https://git-scm.com/docs/git-log>
- Git source and GPL-2.0 project license statement:
  <https://github.com/git/git>
- `gix` 0.85.0 API, feature flags, trust model, and documented integrity
  differences:
  <https://docs.rs/gix/0.85.0/gix/>
- `gix` 0.85.0 rename tracking:
  <https://docs.rs/gix/0.85.0/gix/diff/struct.Options.html>
- `git2` 0.21.0 features, vendored linking behavior, and required `libgit2`
  version:
  <https://docs.rs/crate/git2/0.21.0>
- `git2` 0.21.0 rename API:
  <https://docs.rs/git2/0.21.0/git2/struct.Diff.html>
- `libgit2` implementation scope, build options, native dependencies, and
  license:
  <https://github.com/libgit2/libgit2>
