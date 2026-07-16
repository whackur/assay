import type {
  ProjectComparison,
  ProjectEvaluation,
  ProjectEvidence,
} from "@/lib/contract/types";

// Defensive narrowing of untyped API/fixture JSON into the versioned contract.
// It checks presence and shape of load-bearing fields only; it never fills in
// or derives score values.

export class ContractError extends Error {}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function require(condition: boolean, message: string): asserts condition {
  if (!condition) throw new ContractError(message);
}

export function parseEvaluation(input: unknown): ProjectEvaluation {
  require(isRecord(input), "evaluation must be an object");
  const value = input as Record<string, unknown>;

  require(
    typeof value.schema_version === "string" && value.schema_version.startsWith("1."),
    "unsupported project-evaluation schema_version",
  );
  require(typeof value.status === "string", "evaluation status is required");
  require(typeof value.provisional === "boolean", "evaluation provisional is required");
  require(isRecord(value.scores), "evaluation scores are required");
  require(isRecord(value.evaluator), "evaluation evaluator is required");
  require(isRecord(value.project), "evaluation project is required");

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
  require(isRecord(input), "evidence must be an object");
  const value = input as Record<string, unknown>;
  require(
    typeof value.schema_version === "string" && value.schema_version.startsWith("1."),
    "unsupported project-evidence schema_version",
  );
  require(
    typeof value.id === "string" && value.id.startsWith("evidence:"),
    "evidence id must be an evidence reference",
  );
  require(typeof value.status === "string", "evidence status is required");
  return input as unknown as ProjectEvidence;
}

export function parseComparison(input: unknown): ProjectComparison {
  require(isRecord(input), "comparison must be an object");
  const value = input as Record<string, unknown>;
  require(
    typeof value.schema_version === "string" && value.schema_version.startsWith("1."),
    "unsupported project-comparison schema_version",
  );
  require(
    value.mode === "functional_cohort" || value.mode === "curated_list",
    "comparison mode must be functional_cohort or curated_list",
  );
  require(value.search_depth === "one_depth", "comparison search_depth must be one_depth");
  require(typeof value.status === "string", "comparison status is required");
  require(Array.isArray(value.detailed_candidates), "detailed_candidates must be an array");
  require(
    (value.detailed_candidates as unknown[]).length <= 5,
    "a comparison lists at most five detailed candidates",
  );
  require(Array.isArray(value.additional_candidates), "additional_candidates must be an array");
  return input as unknown as ProjectComparison;
}
