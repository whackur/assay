# Web dashboard

Next.js and TypeScript dashboard for Assay project intelligence. It consumes
the versioned Rust API report contract and renders reports. It never duplicates
metric or score calculation.

## Contract source of truth

The UI types in `src/lib/contract/` mirror the machine-readable schemas under
`schemas/project-evaluation/v1.json` and `schemas/project-evidence/v1.json`.
Machine-readable field names stay snake_case; user-facing text is English.

## No hosted API yet

There is no `assay-api` in this repository. The API client in
`src/lib/api/client.ts` is a thin interface with a fixture-backed default
(`src/lib/api/fixtures.ts`). Fixtures conform to the versioned schemas and drive
development and demos. Swap the implementation for an HTTP transport when the
Rust API exists, without changing the UI.

## Display logic is pure and tested

UI state mapping lives in `src/lib/state/` as pure functions with unit tests:
score-status display, named analysis stages and elapsed time, refresh cooldown,
GitHub URL canonicalization, result badges, and evidence grouping. These map
already-compiled values; they do not compute scores.

## Scripts

```sh
pnpm dev         # local development server
pnpm run lint    # eslint (flat config, eslint-config-next)
pnpm run type-check  # tsc --noEmit
pnpm test        # node --test with tsx over src/**/*.test.ts
pnpm run build   # production build
```

Next.js telemetry is disabled in every script via `NEXT_TELEMETRY_DISABLED=1`.
The app pulls no external CDN, font, or telemetry resources.

## Containerized production preview

From the repository root, run the web dashboard and its production build with
Docker Compose:

```sh
docker compose up --build
```

The dashboard is available at `http://localhost:3000`. Set `ASSAY_WEB_PORT` in
the root `.env` file to change the host port. The current container serves the
fixture-backed web surface; it does not imply that a hosted API or database is
available yet.

## Catalog and badges

Catalog presentation lives in `src/lib/catalog/` as pure functions: entry
projection, public-only filtering, category/tag/engine/score-range filtering,
and Recently Assayed / Top Assays ordering. Featuring is editorial and labeled;
it never affects a score. A score-range filter lists only released scores and
never treats an unavailable score as a zero.

README SVG badges (WEB-003) are a pure function in `src/lib/badge/`. The badge
states the engine profile, score, and provisional/stale/insufficient-evidence
state, is self-contained (no external font or resource), escapes every input,
and is covered by deterministic golden tests under `src/lib/badge/__golden__/`.

## Routes

- `/` — featured cards (Hermes Agent, OpenClaw), catalog filters, Recently
  Assayed and Top Assays lists, the submission form with live canonical
  preview, and the project-not-authors notice.
- `/evaluations/[...slug]` — a fixture record renders either the asynchronous
  progress screen (elapsed time and named stage, no fabricated percentage) or
  the completed result (engine profile, dimension score cards, introduction,
  evidence explorer, the project-comparison/v1 similar-projects cohort, and the
  README badge share block).
- `/badge/[...slug]` — the README SVG badge for a completed result.
- `/contact` — the only feedback path in the first MVP; there is no user-facing
  score editing, retry, reaction, comment, bookmark, follow, or claim.
