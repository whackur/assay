import assert from "node:assert/strict";
import { test } from "node:test";
import type { ProjectEvaluation } from "@/lib/contract/types";
import { isPublicResult, resultState } from "@/lib/state/result-state";

function evaluation(overrides: Partial<ProjectEvaluation>): ProjectEvaluation {
  const base: ProjectEvaluation = {
    schema_version: "1.0.0",
    evaluation_version: "project-intelligence-1",
    status: "complete",
    provisional: false,
    visibility: "public",
    evaluator: {
      profile: "deterministic-project-evaluator-1",
      provider: "deterministic",
      model: null,
      rubric_version: "project-rubric-1",
    },
    compiler: { version: "c-1", rule_set_hash: "sha256:" + "0".repeat(64), judgment_bundle_hash: null },
    project: {
      source: { kind: "hosted", provider: "github", namespace: "o", repository: "r" },
      revision: "0123456789abcdef0123456789abcdef01234567",
    },
    classification: {
      status: "complete",
      primary_type: "cli_developer_tool",
      secondary_types: [],
      tags: [],
      maturity: "prototype",
      confidence: 0.7,
      evidence_ids: ["evidence:repository:snapshot"],
    },
    scores: {
      assay_score: { status: "complete", value: 72, confidence: 0.8, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      project_substance: { status: "complete", value: 70, confidence: 0.8, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      originality: { status: "complete", value: 60, confidence: 0.6, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      engineering_rigor: { status: "complete", value: 80, confidence: 0.7, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      open_source_readiness: { status: "complete", value: 75, confidence: 0.7, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      maintenance_health: { status: "complete", value: 65, confidence: 0.6, version: "v", evidence_ids: ["evidence:repository:snapshot"] },
      potential: { status: "complete", value: 55, confidence: 0.5, version: "v", evidence_ids: ["evidence:repository:snapshot"], forecast_horizon: "P1Y", assumptions: [], major_counter_signals: [] },
    },
    evidence_ids: ["evidence:repository:snapshot"],
    introduction: { status: "complete", factual_statements: [], interpretations: [] },
    warnings: [],
    limitations: [],
  };
  return { ...base, ...overrides };
}

test("anonymous public result is labeled anonymous", () => {
  const s = resultState(evaluation({ visibility: "public" }));
  assert.equal(s.isAnonymous, true);
  assert.equal(s.profileLabel, "Anonymous profile");
  assert.equal(s.visibilityLabel, "Public");
});

test("authenticated result starts as a private preview", () => {
  const s = resultState(evaluation({ visibility: "private_preview" }));
  assert.equal(s.isAnonymous, false);
  assert.equal(s.visibilityLabel, "Private preview");
});

test("provisional and insufficient release gate yield badges", () => {
  const s = resultState(
    evaluation({
      provisional: true,
      scores: {
        ...evaluation({}).scores,
        assay_score: { status: "insufficient", value: null, confidence: 0, version: "v", evidence_ids: [] },
      },
    }),
  );
  assert.deepEqual(s.badges, ["provisional", "insufficient_evidence"]);
  assert.equal(s.releaseGateMet, false);
});

test("only a public result is publicly viewable", () => {
  assert.equal(isPublicResult(evaluation({ visibility: "public" })), true);
  assert.equal(isPublicResult(evaluation({ visibility: "private_preview" })), false);
  assert.equal(isPublicResult(evaluation({ visibility: "private_local" })), false);
});

test("provider-unavailable diagnostic is detected", () => {
  const s = resultState(
    evaluation({
      status: "partial",
      warnings: [{ code: "provider_unavailable", evidence_ids: [] }],
    }),
  );
  assert.equal(s.providerUnavailable, true);
  assert.equal(s.isPartial, true);
});
