import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { parseEvaluation } from "@/lib/contract/parse";

// Fresh output produced by the Rust evaluator -> domain -> score-compiler
// chain and regenerated on every `cargo test` run (see the Rust guard test
// full_chain_evaluation_is_deterministic_and_matches_committed_fixture). This
// is producer output rather than a reviewed golden.
const producedDir = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../../../tests/integration/produced",
);

function produced(name: string): unknown {
  return JSON.parse(readFileSync(join(producedDir, name), "utf8"));
}

test("web contract parser accepts fresh score-compiler output", () => {
  const evaluation = parseEvaluation(produced("project-evaluation.json"));
  assert.equal(evaluation.evaluation_version, "project-intelligence-1");
  assert.equal(evaluation.status, "partial");
  assert.equal(evaluation.evaluator.rubric_version, "project-rubric-1");
  // The overall score stays unscored while an essential dimension is missing.
  assert.equal(evaluation.scores.assay_score.status, "insufficient");
  assert.equal(evaluation.scores.assay_score.value, null);
  // Potential is a separate forecast and never folds into the Assay Score.
  assert.equal(evaluation.scores.potential.forecast_horizon, "P1Y");
});
