import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ANALYSIS_STAGES,
  formatElapsed,
  isStageComplete,
  stageIndex,
  stageLabel,
} from "@/lib/state/stages";

test("stages are ordered from queued to publishing", () => {
  assert.equal(ANALYSIS_STAGES[0], "queued");
  assert.equal(ANALYSIS_STAGES[ANALYSIS_STAGES.length - 1], "publishing");
});

test("completed stages precede the current stage", () => {
  assert.equal(isStageComplete("collecting", "evaluating"), true);
  assert.equal(isStageComplete("evaluating", "evaluating"), false);
  assert.equal(isStageComplete("publishing", "evaluating"), false);
  assert.ok(stageIndex("compiling") > stageIndex("classifying"));
});

test("stage labels are human readable", () => {
  assert.equal(stageLabel("evaluating"), "AI evidence-rubric evaluation");
});

test("elapsed time formats without any percentage", () => {
  assert.equal(formatElapsed(0), "0:00");
  assert.equal(formatElapsed(7_000), "0:07");
  assert.equal(formatElapsed(83_000), "1:23");
  assert.equal(formatElapsed(3_723_000), "1:02:03");
  assert.equal(formatElapsed(-500), "0:00");
});
