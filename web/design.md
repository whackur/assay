# Design — Assay

A locked design system for this app. Every page redesign reads this file before
emitting code. Do not regenerate per page — extend or amend this file when the
system needs to grow.

## Voice

"The assay bench." Assaying is the laboratory testing of metal for purity;
the interface borrows that register at full voltage: a near-black bench, one
electric molten-copper accent used ruthlessly, paper specimen sheets, and
monospaced provenance. The front door provokes ("How good is your code,
really?"); the report proves — that contrast is deliberate. Confident,
slightly cocky, never cute; provocative is never tacky (no gradients, no
emoji, no fake urgency). The web never overclaims — the fixture/preview
state is stated in plain sight.

## Genre

editorial (technical lab-report voice)

## Macrostructure family

- Marketing/landing (`/`): Workbench, adapted — the real rendered report is
  the primary content (no fake browser chrome, no screenshots; the product IS
  a web report, so it is rendered live from fixtures).
- App pages (`/evaluations/*`, `/admin/*`, `/setup`): report/document layout —
  masthead + hairline-ruled sections. No enrichment.
- Content pages (`/contact`): typography only, narrow measure.

## Theme (custom — "assay bench")

Single committed dark theme; `color-scheme: dark`. Tokens in
`src/app/tokens.css`:

- `--color-paper`   oklch(17.5% 0.012 50)  (the bench — warm near-black)
- `--color-paper-2` oklch(21.5% 0.014 50)
- `--color-ink`     oklch(93% 0.012 85)
- `--color-ink-2`   oklch(69% 0.015 70)
- `--color-rule`    oklch(31% 0.015 55)
- `--color-accent`  oklch(72% 0.17 45)  (electric molten copper)
- `--color-focus`   oklch(76% 0.12 250)

The `.sheet` scope re-declares the full token set in a light paper voice: a
specimen report is a paper document lying on the dark bench. Status colors
(`--color-ok/warn/bad`) are reserved for state and always paired with a text
label.

## Typography

- Display: Fraunces (variable, opsz), weight ~540–600. Headings, hero numerals.
- Body: IBM Plex Sans 400/500/600.
- Mono: IBM Plex Mono 400/500 — evidence ids, hashes, timestamps, form labels,
  scores, chips. The mono voice marks "data from the contract".
- Scale anchor: `--text-display: clamp(2.5rem, 6vw, 4.1rem)`.

## Spacing

4-point named scale (`--space-3xs … --space-3xl`) in tokens.css. Pages must use
named tokens, never raw values.

## Motion

- Easing: `--ease-out: cubic-bezier(0.16, 1, 0.3, 1)`; duration `--dur-short`.
- Two intentional animations only: the progress screen's current-stage pulse
  (activity) and the score count-up on reveal (~700ms, the verdict settling).
  No scroll reveals, no entrance animations.
- Reduced motion: pulse removed, count-up skipped to the final value,
  transitions collapse to 1ms.

## Microinteractions stance

- Silent success; the copy button's inline "Copied" text is the only success
  confirmation in the app.
- Focus rings appear instantly, `--color-focus`, 2px, offset 2px.
- Buttons: background shift on hover, 1px translate on :active.

## CTA voice

- Primary: copper fill, near-flat 4px radius, mono-adjacent short labels
  ("Assay it", "Sign in", "Create admin account").
- Secondary (`button.quiet`): hairline outline, paper fill.

## Data visualization

- Dimension scores: single-hue thin bars (6px) on a track, direct value labels
  in tabular mono. Unreleased scores render as an em dash + status, never a
  zero bar.
- Assay Score: hero number in Fraunces copper, `/ 100` denominator in mono.
- No categorical multi-series palettes anywhere; if one is ever needed, run
  the dataviz palette validator first.

## What pages MUST share

- The wordmark, masthead, colophon footer.
- The copper accent (≤ 5% of any viewport) and paper/ink neutrals.
- Fraunces + IBM Plex pairing and the mono "data voice".
- Hairline rules as the section divider language.
- Honest-capability notes wherever fixture/preview data is shown.

## What pages MAY differ on

- Landing may render the sample report sheet ("specimen") and trace chain.
- Report pages use the report masthead + section rhythm.
- Admin uses the utilitarian table voice; no enrichment ever.
