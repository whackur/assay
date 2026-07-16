import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  ContractError,
  parseComparison,
  parseEvaluation,
  parseEvidence,
} from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real project-evaluation golden fixture", () => {
  const evaluation = parseEvaluation(golden("project-evaluation-v1.json"));
  assert.equal(evaluation.status, "partial");
  assert.equal(evaluation.scores.assay_score.status, "insufficient");
  assert.equal(evaluation.scores.assay_score.value, null);
  assert.equal(evaluation.scores.potential.forecast_horizon, "P1Y");
});

test("parses the real project-evidence golden fixture", () => {
  const evidence = parseEvidence(golden("project-evidence-v1.json"));
  assert.equal(evidence.id, "evidence:file:src-main-ts");
  assert.equal(evidence.grade, "a");
});

test("rejects an unsupported schema version", () => {
  assert.throws(
    () => parseEvaluation({ schema_version: "2.0.0" }),
    ContractError,
  );
});

test("parses the real project-comparison golden fixture", () => {
  const comparison = parseComparison(golden("project-comparison-v1.json"));
  assert.equal(comparison.mode, "functional_cohort");
  assert.equal(comparison.search_depth, "one_depth");
  assert.equal(comparison.status, "partial");
  assert.equal(comparison.detailed_candidates.length, 5);
});

test("rejects a comparison with more than five detailed candidates", () => {
  assert.throws(
    () =>
      parseComparison({
        schema_version: "1.0.0",
        mode: "functional_cohort",
        search_depth: "one_depth",
        status: "complete",
        detailed_candidates: new Array(6).fill({}),
        additional_candidates: [],
      }),
    ContractError,
  );
});
