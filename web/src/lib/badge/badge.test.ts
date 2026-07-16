import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import type { BadgeInput } from "@/lib/badge/badge";
import { badgeInput, renderBadge } from "@/lib/badge/badge";
import type { ProjectEvaluation } from "@/lib/contract/types";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "__golden__");

function golden(name: string): string {
  return readFileSync(join(goldenDir, `${name}.svg`), "utf8");
}

const GOLDEN_CASES: Record<string, BadgeInput> = {
  scored: { label: "assay: OpenAI API", message: "82/100", tone: "ok" },
  provisional: { label: "assay: OpenAI API", message: "58/100 provisional", tone: "warn" },
  insufficient: { label: "assay: Deterministic", message: "insufficient evidence", tone: "neutral" },
};

for (const [name, input] of Object.entries(GOLDEN_CASES)) {
  test(`renderBadge matches the ${name} golden`, () => {
    assert.equal(renderBadge(input), golden(name));
  });
}

test("renderBadge escapes markup in every text field", () => {
  const svg = renderBadge({ label: 'a<b>&"', message: "</svg><script>", tone: "ok" });
  assert.ok(!svg.includes("<script>"));
  assert.ok(!svg.includes("</svg><script>"));
  assert.ok(svg.includes("&lt;script&gt;"));
  assert.ok(svg.includes("&amp;"));
  assert.equal(svg.lastIndexOf("</svg>"), svg.length - "</svg>".length);
});

function evaluation(partial: Partial<ProjectEvaluation>): ProjectEvaluation {
  return {
    schema_version: "1.0.0",
    evaluation_version: "project-intelligence-1",
    status: "complete",
    provisional: false,
    visibility: "public",
    evaluator: { profile: "p", provider: "openai_api", model: "m", rubric_version: "r" },
    compiler: { version: "c", rule_set_hash: "sha256:" + "a".repeat(64), judgment_bundle_hash: null },
    project: {
      source: { kind: "hosted", provider: "github", namespace: "o", repository: "r" },
      revision: "0123456789abcdef0123456789abcdef01234567",
    },
    classification: {
      status: "complete",
      primary_type: "cli_developer_tool",
      secondary_types: [],
      tags: [],
      maturity: "beta",
      confidence: 0.8,
      evidence_ids: ["evidence:repository:snapshot"],
    },
    scores: {
      assay_score: {
        status: "complete",
        value: 82,
        confidence: 0.7,
        version: "v",
        evidence_ids: ["evidence:repository:snapshot"],
      },
    } as unknown as ProjectEvaluation["scores"],
    evidence_ids: [],
    introduction: { status: "complete", factual_statements: [], interpretations: [] },
    warnings: [],
    limitations: [],
    ...partial,
  };
}

test("badgeInput reports a released score with the ok tone", () => {
  const input = badgeInput(evaluation({}));
  assert.equal(input.message, "82/100");
  assert.equal(input.tone, "ok");
  assert.equal(input.label, "assay: OpenAI API");
});

test("badgeInput marks a provisional score with the warn tone", () => {
  const input = badgeInput(evaluation({ provisional: true }));
  assert.equal(input.message, "82/100 provisional");
  assert.equal(input.tone, "warn");
});

test("badgeInput reports a stale snapshot", () => {
  const input = badgeInput(
    evaluation({ warnings: [{ code: "stale_snapshot", evidence_ids: [] }] }),
  );
  assert.equal(input.message, "82/100 stale");
  assert.equal(input.tone, "warn");
});

test("badgeInput never shows a zero for an unavailable score", () => {
  const input = badgeInput(
    evaluation({
      scores: {
        assay_score: {
          status: "insufficient",
          value: null,
          confidence: 0,
          version: "v",
          evidence_ids: [],
        },
      } as unknown as ProjectEvaluation["scores"],
    }),
  );
  assert.equal(input.message, "insufficient evidence");
  assert.equal(input.tone, "neutral");
});
