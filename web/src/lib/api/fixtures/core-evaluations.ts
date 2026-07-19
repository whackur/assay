// Core ProjectEvaluation fixtures: the canonical scored case, the anonymous
// public partial, the authenticated degraded preview, the recent anonymous
// refresh-cooldown case, and the in-flight job source.

import type { ProjectEvaluation } from "@/lib/contract/types";
import { REVISION, SNAPSHOT, score, source } from "./builders";

export const scoredRepo = source("example-org", "sample-project");
export const scoredEvaluation: ProjectEvaluation = {
  schema_version: "1.0.0",
  evaluation_version: "project-intelligence-1",
  status: "complete",
  provisional: false,
  visibility: "public",
  evaluator: {
    profile: "openai-project-evaluator-1",
    provider: "openai_api",
    model: "gpt-project-eval",
    rubric_version: "project-rubric-1",
  },
  compiler: {
    version: "project-score-compiler-1",
    rule_set_hash: "sha256:" + "a".repeat(64),
    judgment_bundle_hash: "sha256:" + "b".repeat(64),
  },
  project: { source: scoredRepo, revision: REVISION },
  classification: {
    status: "complete",
    primary_type: "cli_developer_tool",
    secondary_types: ["library_sdk_framework"],
    tags: ["developer_tool"],
    maturity: "beta",
    confidence: 0.82,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(74, 0.72),
    project_substance: score(78, 0.8),
    originality: score(61, 0.55),
    engineering_rigor: score(81, 0.7),
    open_source_readiness: score(69, 0.66),
    maintenance_health: score(64, 0.6),
    potential: {
      ...score(58, 0.5),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [
        { text: "Release cadence continues at the current pace.", evidence_ids: [SNAPSHOT] },
      ],
      major_counter_signals: [
        { text: "A single maintainer authors most changes.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  evidence_ids: [SNAPSHOT],
  introduction: {
    status: "complete",
    factual_statements: [
      { text: "The project ships a command-line interface with JSON output.", evidence_ids: [SNAPSHOT] },
    ],
    interpretations: [
      { text: "Its scope targets developers automating repository analysis.", evidence_ids: [SNAPSHOT] },
    ],
  },
  warnings: [],
  limitations: [
    { code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] },
  ],
};

// The anonymous public golden case: partial, insufficient release gate.
export const partialRepo = source("example-org", "early-prototype");
export const partialEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  status: "partial",
  visibility: "public",
  evaluator: {
    profile: "deterministic-project-evaluator-1",
    provider: "deterministic",
    model: null,
    rubric_version: "project-rubric-1",
  },
  project: { source: partialRepo, revision: REVISION },
  classification: {
    status: "complete",
    primary_type: "experimental_proof_of_concept",
    secondary_types: [],
    tags: ["prototype"],
    maturity: "prototype",
    confidence: 0.6,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(null, 0, "insufficient"),
    project_substance: score(null, 0, "insufficient"),
    originality: score(null, 0, "unavailable"),
    engineering_rigor: score(null, 0, "insufficient"),
    open_source_readiness: score(null, 0, "insufficient"),
    maintenance_health: score(null, 0, "unavailable"),
    potential: {
      ...score(null, 0, "unavailable"),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [],
      major_counter_signals: [
        { text: "The current evidence is insufficient for a numeric forecast.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  introduction: { status: "unavailable", factual_statements: [], interpretations: [] },
  warnings: [{ code: "score_release_gate_not_met", evidence_ids: [] }],
  limitations: [{ code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] }],
};

// Authenticated private preview, provisional and provider-degraded.
export const degradedRepo = source("acme", "degraded");
export const degradedEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  status: "partial",
  provisional: true,
  visibility: "private_preview",
  project: { source: degradedRepo, revision: REVISION },
  scores: {
    ...scoredEvaluation.scores,
    originality: score(null, 0, "unavailable"),
    potential: {
      ...scoredEvaluation.scores.potential,
      status: "unavailable",
      value: null,
      evidence_ids: [],
    },
  },
  warnings: [{ code: "provider_unavailable", evidence_ids: [] }],
  limitations: [{ code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] }],
};

// A recently analyzed anonymous public result. A duplicate submission inside
// the 14-day anonymous refresh cooldown cannot force a new run (spec 12.3).
export const recentRepo = source("acme", "recently-analyzed");
export const recentEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  project: { source: recentRepo, revision: REVISION },
};

export const inFlightRepo = source("acme", "in-progress");