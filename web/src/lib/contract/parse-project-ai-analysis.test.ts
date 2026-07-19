import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseProjectAiAnalysis } from "@/lib/contract/parse";

const goldenPath = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden/project-ai-analysis-v1.json");

function golden(): unknown {
  return JSON.parse(readFileSync(goldenPath, "utf8"));
}

test("parses the public project AI analysis golden contract", () => {
  const analysis = parseProjectAiAnalysis(golden());
  assert.equal(analysis.data.judgments[0]?.criterion_id, "quality.documentation");
  assert.equal(analysis.data.judgments[0]?.rating, 3);
  assert.equal(analysis.data.judgments[0]?.rating_scale, 4);
  assert.equal(analysis.data.interpretation.not_a_project_score, true);
});

test("rejects an AI analysis that is missing cited evidence", () => {
  const fixture = structuredClone(golden()) as { data: { judgments: Array<Record<string, unknown>> } };
  fixture.data.judgments[0]!.evidence_ids = ["not-an-evidence-id"];
  assert.throws(() => parseProjectAiAnalysis(fixture), ContractError);
});

test("rejects an AI analysis that changes the score boundary marker", () => {
  const fixture = structuredClone(golden()) as { data: { interpretation: Record<string, unknown> } };
  fixture.data.interpretation.not_a_project_score = false;
  assert.throws(() => parseProjectAiAnalysis(fixture), ContractError);
});
