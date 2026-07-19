// Defensive narrowing of untyped API/fixture JSON into the versioned contract.
// It checks presence and shape of load-bearing fields only; it never fills in
// or derives score values. Per-schema parsers live in ./parse-* modules and
// re-export here so callers keep the "@/lib/contract/parse" import path.

import type {
  ProjectComparison,
  ProjectEvaluation,
  ProjectEvidence,
} from "@/lib/contract/types";
import {
  asRecord,
  isRecord,
  require,
  requireArray,
  requireBoolean,
  requireRecord,
  requireString,
  requireSchemaVersion,
} from "./parse-helpers";
import { parseAnalysis } from "./parse-project-analysis";
import { parseManifest } from "./parse-analysis-manifest";
import { parseRunState } from "./parse-run-state";
import { parseCapabilities } from "./parse-capabilities";
import { parseAiJudgment } from "./parse-ai-judgment";

export { ContractError } from "./parse-helpers";
export { parseAnalysis, parseManifest, parseRunState, parseCapabilities, parseAiJudgment };

export function parseEvaluation(input: unknown): ProjectEvaluation {
  const value = asRecord(input, "evaluation");
  requireSchemaVersion(value, "project-evaluation");
  requireString(value, "status", "evaluation");
  requireBoolean(value, "provisional", "evaluation");
  requireRecord(value, "scores", "evaluation");
  requireRecord(value, "evaluator", "evaluation");
  requireRecord(value, "project", "evaluation");

  const scores = value.scores as Record<string, unknown>;
  for (const dimension of [
    "assay_score",
    "project_substance",
    "originality",
    "engineering_rigor",
    "open_source_readiness",
    "maintenance_health",
    "potential",
  ]) {
    require(isRecord(scores[dimension]), `missing score dimension: ${dimension}`);
  }
  return input as unknown as ProjectEvaluation;
}

export function parseEvidence(input: unknown): ProjectEvidence {
  const value = asRecord(input, "evidence");
  requireSchemaVersion(value, "project-evidence");
  require(
    typeof value.id === "string" && value.id.startsWith("evidence:"),
    "evidence id must be an evidence reference",
  );
  requireString(value, "status", "evidence");
  return input as unknown as ProjectEvidence;
}

export function parseComparison(input: unknown): ProjectComparison {
  const value = asRecord(input, "comparison");
  requireSchemaVersion(value, "project-comparison");
  require(
    value.mode === "functional_cohort" || value.mode === "curated_list",
    "comparison mode must be functional_cohort or curated_list",
  );
  require(value.search_depth === "one_depth", "comparison search_depth must be one_depth");
  requireString(value, "status", "comparison");
  const detailed = requireArray(value, "detailed_candidates", "comparison");
  require(detailed.length <= 5, "a comparison lists at most five detailed candidates");
  requireArray(value, "additional_candidates", "comparison");
  return input as unknown as ProjectComparison;
}