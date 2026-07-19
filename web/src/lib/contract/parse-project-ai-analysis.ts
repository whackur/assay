import type { ProjectAiAnalysisEnvelope } from "@/lib/contract/types";
import {
  asRecord,
  isEvidenceId,
  require,
  requireArray,
  requireRecord,
} from "./parse-helpers";

const APPLICABILITY = new Set([
  "applicable",
  "partially_applicable",
  "not_applicable",
]);

export function parseProjectAiAnalysis(input: unknown): ProjectAiAnalysisEnvelope {
  const value = asRecord(input, "project-ai-analysis");
  require(value.contract === "project-ai-analysis", "project-ai-analysis contract is invalid");
  require(value.schema_version === "1.0.0", "unsupported project-ai-analysis schema_version");
  const data = requireRecord(value, "data", "project-ai-analysis");
  const project = requireRecord(data, "project", "project-ai-analysis");
  requireBoundedString(project.owner, 1, 39, "project owner");
  requireBoundedString(project.repository, 1, 100, "project repository");
  const revision = requireRecord(data, "revision", "project-ai-analysis");
  require(/^[0-9a-f]{40}$/.test(String(revision.commit_sha)), "project commit_sha is invalid");
  requireBoundedString(revision.default_branch, 1, 255, "project default_branch");
  const evaluation = requireRecord(data, "evaluation", "project-ai-analysis");
  requireBoundedString(evaluation.evaluation_version, 1, 100, "evaluation_version");
  requireBoundedString(evaluation.rubric_version, 1, 100, "rubric_version");
  require(typeof evaluation.evidence_bundle_hash === "string" && /^sha256:[0-9a-f]{64}$/.test(evaluation.evidence_bundle_hash), "evidence_bundle_hash is invalid");
  const interpretation = requireRecord(data, "interpretation", "project-ai-analysis");
  require(interpretation.kind === "ai_interpretation" && interpretation.not_a_project_score === true, "interpretation markers are invalid");
  const judgments = requireArray(data, "judgments", "project-ai-analysis");
  require(judgments.length >= 1 && judgments.length <= 64, "project-ai-analysis judgments are invalid");
  judgments.forEach(requireJudgment);
  const limitations = requireArray(data, "limitations", "project-ai-analysis");
  require(limitations.length >= 1 && limitations.length <= 16, "project-ai-analysis limitations are invalid");
  limitations.forEach((item) => requireBoundedString(item, 1, 300, "project-ai-analysis limitation"));
  return input as ProjectAiAnalysisEnvelope;
}

function requireBoundedString(value: unknown, min: number, max: number, label: string): void {
  require(typeof value === "string" && value.length >= min && value.length <= max, `${label} is invalid`);
}

function requireJudgment(input: unknown): void {
  const judgment = asRecord(input, "project-ai-analysis judgment");
  requireBoundedString(judgment.criterion_id, 3, 100, "criterion_id");
  require(typeof judgment.applicability === "string" && APPLICABILITY.has(judgment.applicability), "applicability is invalid");
  require(judgment.rating === null || (typeof judgment.rating === "number" && Number.isInteger(judgment.rating) && judgment.rating >= 0 && judgment.rating <= 4), "rating is invalid");
  require(judgment.rating_scale === 4, "rating_scale must be 4");
  require(typeof judgment.confidence === "number" && Number.isFinite(judgment.confidence) && judgment.confidence >= 0 && judgment.confidence <= 1, "confidence is invalid");
  const evidenceIds = requireArray(judgment, "evidence_ids", "project-ai-analysis judgment");
  require(evidenceIds.length <= 32 && new Set(evidenceIds).size === evidenceIds.length, "evidence_ids are invalid");
  evidenceIds.forEach((id) => require(isEvidenceId(id), "evidence_id is invalid"));
  requireBoundedString(judgment.rationale, 1, 1000, "rationale");
}
