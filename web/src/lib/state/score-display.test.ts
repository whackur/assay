import assert from "node:assert/strict";
import { test } from "node:test";
import type { Score } from "@/lib/contract/types";
import { confidenceBand, scoreDisplay } from "@/lib/state/score-display";

function score(partial: Partial<Score>): Score {
  return {
    status: "complete",
    value: 72,
    confidence: 0.8,
    version: "project-score-1",
    evidence_ids: ["evidence:repository:snapshot"],
    ...partial,
  };
}

test("complete score shows numeric value and confidence", () => {
  const d = scoreDisplay(score({ value: 72, confidence: 0.8 }));
  assert.equal(d.hasValue, true);
  assert.equal(d.valueText, "72");
  assert.equal(d.confidencePercent, 80);
  assert.equal(d.confidenceBand, "high");
  assert.equal(d.statusLabel, "Scored");
});

test("insufficient score hides value and confidence", () => {
  const d = scoreDisplay(score({ status: "insufficient", value: null, confidence: 0 }));
  assert.equal(d.hasValue, false);
  assert.equal(d.valueText, "—");
  assert.equal(d.confidencePercent, null);
  assert.equal(d.confidenceBand, null);
  assert.equal(d.statusLabel, "Insufficient evidence");
});

test("unavailable never renders a zero that reads as a poor score", () => {
  const d = scoreDisplay(score({ status: "unavailable", value: null, confidence: 0 }));
  assert.equal(d.valueText, "—");
  assert.equal(d.statusLabel, "Not available");
});

test("confidence band thresholds", () => {
  assert.equal(confidenceBand(0), "low");
  assert.equal(confidenceBand(0.32), "low");
  assert.equal(confidenceBand(0.33), "medium");
  assert.equal(confidenceBand(0.65), "medium");
  assert.equal(confidenceBand(0.66), "high");
  assert.equal(confidenceBand(1), "high");
});
