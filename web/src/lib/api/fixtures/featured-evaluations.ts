// Featured catalog card evaluation fixtures. Editorial featuring is
// independent of the score (specification 13); the cards are visibly labeled
// as featured. OpenClaw's provisional score exercises the labeled
// low-confidence state in the Top Assays list.

import type { ProjectEvaluation } from "@/lib/contract/types";
import { REVISION, SNAPSHOT, score, source } from "./builders";
import { scoredEvaluation } from "./core-evaluations";

// Featured catalog card: Hermes Agent.
export const hermesRepo = source("hermes-agent", "hermes");
export const hermesEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  project: { source: hermesRepo, revision: REVISION },
  classification: {
    status: "complete",
    primary_type: "cli_developer_tool",
    secondary_types: ["service_infrastructure_platform"],
    tags: ["agent", "automation", "developer_tool"],
    maturity: "stable",
    confidence: 0.88,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(88, 0.82),
    project_substance: score(90, 0.85),
    originality: score(72, 0.6),
    engineering_rigor: score(86, 0.78),
    open_source_readiness: score(84, 0.74),
    maintenance_health: score(83, 0.7),
    potential: {
      ...score(80, 0.62),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [
        { text: "The gateway and skill surfaces keep their release cadence.", evidence_ids: [SNAPSHOT] },
      ],
      major_counter_signals: [
        { text: "Breadth of integrations raises long-term maintenance load.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  introduction: {
    status: "complete",
    factual_statements: [
      { text: "Hermes Agent is a multi-gateway automation agent with a skill and tool system.", evidence_ids: [SNAPSHOT] },
    ],
    interpretations: [
      { text: "It targets operators wiring language-model agents into chat and workflow platforms.", evidence_ids: [SNAPSHOT] },
    ],
  },
};

// Featured catalog card: OpenClaw. A provisional score exercises the labeled
// low-confidence state in the Top Assays list.
export const openclawRepo = source("openclaw", "openclaw");
export const openclawEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  provisional: true,
  project: { source: openclawRepo, revision: REVISION },
  evaluator: {
    profile: "deterministic-project-evaluator-1",
    provider: "deterministic",
    model: null,
    rubric_version: "project-rubric-1",
  },
  compiler: {
    version: "project-score-compiler-1",
    rule_set_hash: "sha256:" + "a".repeat(64),
    judgment_bundle_hash: null,
  },
  classification: {
    status: "complete",
    primary_type: "application",
    secondary_types: ["cli_developer_tool"],
    tags: ["agent", "game"],
    maturity: "beta",
    confidence: 0.64,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(66, 0.42),
    project_substance: score(70, 0.5),
    originality: score(74, 0.55),
    engineering_rigor: score(62, 0.44),
    open_source_readiness: score(58, 0.4),
    maintenance_health: score(60, 0.4),
    potential: {
      ...score(64, 0.4),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [
        { text: "Early contributor interest continues to grow.", evidence_ids: [SNAPSHOT] },
      ],
      major_counter_signals: [
        { text: "The release history is short, so durability is not yet demonstrated.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  introduction: {
    status: "complete",
    factual_statements: [
      { text: "OpenClaw is an autonomous agent runtime with a command-line entry point.", evidence_ids: [SNAPSHOT] },
    ],
    interpretations: [
      { text: "It aims at developers experimenting with self-directed task agents.", evidence_ids: [SNAPSHOT] },
    ],
  },
};