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
npm run dev         # local development server
npm run lint        # eslint (flat config, eslint-config-next)
npm run type-check  # tsc --noEmit
npm run test        # node --test with tsx over src/**/*.test.ts
npm run build       # production build
```

Next.js telemetry is disabled in every script via `NEXT_TELEMETRY_DISABLED=1`.
The app pulls no external CDN, font, or telemetry resources.

## Routes

- `/` — submission form, live canonical preview, and the project-not-authors
  notice.
- `/evaluations/[...slug]` — a fixture record renders either the asynchronous
  progress screen (elapsed time and named stage, no fabricated percentage) or
  the completed result (engine profile, dimension score cards, introduction,
  and evidence explorer).

## Out of scope

README SVG badges (WEB-003), authentication wiring (IAM-001), and the catalog
home (CAT-001) are separate cards. Similar-project comparison rendering is
deferred to the detail/catalog work.
