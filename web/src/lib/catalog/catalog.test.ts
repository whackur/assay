import assert from "node:assert/strict";
import { test } from "node:test";
import {
  filterEntries,
  filterOptions,
  matchesFilter,
  recentlyAssayed,
  topAssays,
  type CatalogEntry,
} from "@/lib/catalog/catalog";
import { featuredEntries, publicCatalogEntries } from "@/lib/catalog/fixtures";

function entry(partial: Partial<CatalogEntry>): CatalogEntry {
  return {
    id: "o/r",
    name: "o/r",
    canonicalUrl: "https://github.com/o/r",
    description: "d",
    primaryType: "cli_developer_tool",
    tags: ["developer_tool"],
    maturity: "beta",
    provider: "openai_api",
    engineLabel: "OpenAI API",
    profile: "p",
    evaluationVersion: "project-intelligence-1",
    provisional: false,
    visibility: "public",
    assayedAt: "2026-07-14T00:00:00Z",
    score: { status: "complete", value: 70, confidence: 0.7 },
    badges: [],
    featuredLabel: null,
    ...partial,
  };
}

test("the public catalog excludes non-public results", () => {
  const entries = publicCatalogEntries();
  assert.ok(entries.length > 0);
  assert.ok(entries.every((e) => e.visibility === "public"));
  assert.ok(!entries.some((e) => e.id === "acme/degraded"));
});

test("both featured cards are present and labeled", () => {
  const featured = featuredEntries();
  const labels = featured.map((e) => e.featuredLabel).sort();
  assert.deepEqual(labels, ["Hermes Agent", "OpenClaw"]);
});

test("recently assayed orders by assay time descending", () => {
  const ordered = recentlyAssayed(publicCatalogEntries());
  assert.equal(ordered[0]!.id, "hermes-agent/hermes");
  for (let i = 1; i < ordered.length; i++) {
    assert.ok(Date.parse(ordered[i - 1]!.assayedAt) >= Date.parse(ordered[i]!.assayedAt));
  }
});

test("top assays ranks released scores and omits unavailable ones", () => {
  const ranked = topAssays(publicCatalogEntries());
  assert.ok(ranked.every((e) => e.score.value !== null));
  assert.equal(ranked[0]!.id, "hermes-agent/hermes");
  // early-prototype has an insufficient assay score; it must not be ranked.
  assert.ok(!ranked.some((e) => e.id === "example-org/early-prototype"));
});

test("category filter matches primary type", () => {
  const entries = [entry({ id: "a", primaryType: "application" }), entry({ id: "b" })];
  const out = filterEntries(entries, { primaryType: "application" });
  assert.deepEqual(out.map((e) => e.id), ["a"]);
});

test("tag filter matches an included tag", () => {
  const entries = [entry({ id: "a", tags: ["agent"] }), entry({ id: "b", tags: ["cli"] })];
  assert.deepEqual(filterEntries(entries, { tag: "agent" }).map((e) => e.id), ["a"]);
});

test("engine filter matches provider", () => {
  const entries = [entry({ id: "a", provider: "deterministic" }), entry({ id: "b" })];
  assert.deepEqual(filterEntries(entries, { provider: "deterministic" }).map((e) => e.id), ["a"]);
});

test("score-range filter excludes unavailable scores instead of treating them as zero", () => {
  const unavailable = entry({ id: "u", score: { status: "insufficient", value: null, confidence: 0 } });
  const low = entry({ id: "low", score: { status: "complete", value: 20, confidence: 0.5 } });
  const high = entry({ id: "high", score: { status: "complete", value: 90, confidence: 0.8 } });
  const out = filterEntries([unavailable, low, high], { minScore: 50 });
  assert.deepEqual(out.map((e) => e.id), ["high"]);
  assert.ok(!matchesFilter(unavailable, { minScore: 0 }));
});

test("filter options are the distinct sorted facets across entries", () => {
  const options = filterOptions([
    entry({ primaryType: "application", tags: ["b", "a"], provider: "openai_api" }),
    entry({ primaryType: "cli_developer_tool", tags: ["a"], provider: "deterministic" }),
  ]);
  assert.deepEqual(options.primaryTypes, ["application", "cli_developer_tool"]);
  assert.deepEqual(options.tags, ["a", "b"]);
  assert.deepEqual(options.providers, ["deterministic", "openai_api"]);
});
