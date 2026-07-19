# ADR 0015: Windows CI Gate and Test Portability

- Status: Accepted
- Date: 2026-07-19

## Context

ADR 0002 records that the installed Git CLI adapter decision should be reopened
when "self-contained Linux, macOS, or Windows binaries become a release gate."
The hosted ingestion handoff (2026-07-18) and the Windows toolchain handoff
(2026-07-16) both ask whether `.github/workflows/ci.yml` should add a Windows
runner job after the WIN-001/002/003 portability fixes land.

Before this decision, CI only ran on `ubuntu-24.04`. The Windows toolchain
handoff documented three portability defect classes (A: Unix-only Git
executable resolution in test helpers, B: Unix-style absolute paths in missing
executable probes, C: `env_clear()` stripping `SystemRoot` and breaking
Winsock). All three were fixed in commits `5742aff` and `efbbbf7`, and the
300-LOC test split completed the verification surface.

## Decision

Add a Windows CI job that runs the same Rust gates as the Linux job, plus the
single-threaded `assay-git` retry documented as FLAKY-001. Do not add a Windows
web job yet: the web dashboard builds and tests pass on Windows locally, but
the hosted web stack targets Linux containers in production, so a Windows web
gate adds cost without protecting a production target.

The Windows job is a release gate, not advisory. A regression that only
reproduces on Windows must fail CI before it reaches `main`.

## Decision drivers

1. The CLI is a native `x86_64-pc-windows-msvc` binary that operators run on
   Windows workstations. A Linux-only gate cannot catch Winsock, path, or
   permission regressions.
2. ADR 0002 rule 1 already requires platform-specific trusted Git resolution.
   CI is the cheapest place to keep that contract honest.
3. The 300-LOC test split removed the last structural excuse for skipping
   Windows: every helper module and test file now compiles and runs on both
   platforms.
4. The FLAKY-001 `assay-git` ProbeCapabilities/Io failures are load-sensitive
   on both platforms. Single-threaded retry is the documented mitigation and
   is cheap enough to run in CI.

## Alternatives considered

### Keep Linux-only CI and rely on manual Windows verification

Rejected. The Windows toolchain handoff showed that portability regressions
silently accumulate between manual checks. The cost of a Windows runner minute
is lower than the cost of a Windows-only release blocker discovered late.

### Add Windows jobs for both Rust and web

Rejected for now. The web dashboard production target is a Linux container
(`compose.yaml`, `Dockerfile.hosted`). A Windows web gate would protect the
developer experience but not a production deployment path. Revisit when the
web dashboard ships a native Windows build target.

### Wait until a self-contained Windows binary is a release gate

Rejected. ADR 0002's reopen condition is about the Git adapter choice, not
about CI coverage. Portability coverage is a separate concern that the CLI's
native Windows binary already triggers.

## Consequences

- `.github/workflows/ci.yml` gains a `windows` job with the pinned Rust
  toolchain, `cargo fmt --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace`, and a
  single-threaded `cargo test -p assay-git -- --test-threads=1` retry.
- The Windows job uses `windows-latest` with Git for Windows preinstalled. The
  adapter baseline check (`git version --build-options` and the 2.47.0
  version comparison) runs the same way as on Linux.
- Cache strategy: `actions/cache` keyed on `${{ runner.os }}-cargo-...` already
  separates Windows and Linux caches by OS. No additional key is needed.
- The web job stays Linux-only. A future native Windows web target would
  reopen this decision.
- FLAKY-001 stays tracked separately. If the single-threaded retry stops
  stabilizing the failures, the flaky tests need a real fix, not a CI
  workaround.

## Primary sources

- ADR 0002: Git adapter decision and reopen conditions.
- `docs/internal/handoffs/2026-07-16-windows-toolchain-and-test-portability.md`:
  portability defect classes A/B/C and the FLAKY-001 note.
- `docs/internal/handoffs/2026-07-18-hosted-ingestion.md`: Windows CI job
  review request.
- Commits `5742aff` (test portability fixes) and `efbbbf7` (handoff record).