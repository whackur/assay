import assert from "node:assert/strict";
import { test } from "node:test";
import { hostedEvaluationLabel, hostedPublicationLabel, hostedScoreLabel } from "@/lib/state/hosted-status-display";

test("distinguishes a validated judgment from its unpublished score", () => {
  assert.equal(hostedEvaluationLabel("validated_unpublished"), "Validated judgment (not published)");
  assert.equal(hostedPublicationLabel("validated_unpublished"), "Validated judgment (not published)");
  assert.equal(hostedScoreLabel("unavailable"), "Unavailable");
});

test("reports an auto-published AI analysis", () => {
  assert.equal(hostedEvaluationLabel("validated_published"), "Published AI analysis");
  assert.equal(hostedPublicationLabel("validated_published"), "Published AI analysis");
});

test("keeps other hosted workflow states explicit", () => {
  assert.equal(hostedEvaluationLabel(null), "Not evaluated");
  assert.equal(hostedEvaluationLabel("partial"), "Partially evaluated");
  assert.equal(hostedEvaluationLabel("unavailable"), "Evaluation unavailable");
  assert.equal(hostedScoreLabel("pending"), "Pending");
});
