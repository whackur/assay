// Parser for schemas/ai-judgment/v1.json. Validates the rubric judgment
// invariants: not_applicable judgments carry a null rating, every other
// applicability requires an integer rating and at least one evidence id, and
// complete/partial bundles carry at least one judgment.

import type { AiJudgment } from "@/lib/contract/types";
import {
  asRecord,
  isSha256,
  isVersionIdentifier,
  require,
  requireArray,
  requireRecord,
  requireString,
  requireSchemaVersion,
} from "./parse-helpers";

const STATUS_VALUES = new Set([
  "complete",
  "partial",
  "unavailable",
  "unsupported",
  "insufficient",
  "pending",
]);
const APPLICABILITY = new Set([
  "applicable",
  "partially_applicable",
  "not_applicable",
]);
const EVIDENCE_SCOPE = new Set(["public_only", "private_local"]);
const EXTERNAL_TRANSMISSION = new Set(["not_used", "public_only", "consented_private"]);

export function parseAiJudgment(input: unknown): AiJudgment {
  const value = asRecord(input, "ai-judgment");
  requireSchemaVersion(value, "ai-judgment");
  requireString(value, "evaluation_version", "ai-judgment");
  require(isVersionIdentifier(value.evaluation_version), "evaluation_version is malformed");
  requireString(value, "rubric_version", "ai-judgment");
  require(isVersionIdentifier(value.rubric_version), "rubric_version is malformed");
  require(typeof value.status === "string" && STATUS_VALUES.has(value.status), "ai-judgment status is invalid");
  require(isSha256(value.evidence_bundle_hash), "evidence_bundle_hash must be a sha256");
  requireRecord(value, "privacy", "ai-judgment");
  requirePrivacy(value.privacy);
  const judgments = requireArray(value, "judgments", "ai-judgment");
  for (const j of judgments) requireJudgment(j);
  if (value.status === "complete" || value.status === "partial") {
    require(judgments.length >= 1, "complete/partial ai-judgment has at least one judgment");
  }
  return input as unknown as AiJudgment;
}

function requirePrivacy(input: unknown): void {
  const p = asRecord(input, "privacy");
  require(typeof p.evidence_scope === "string" && EVIDENCE_SCOPE.has(p.evidence_scope), "privacy evidence_scope is invalid");
  require(
    typeof p.external_transmission === "string" && EXTERNAL_TRANSMISSION.has(p.external_transmission),
    "privacy external_transmission is invalid",
  );
  if (p.evidence_scope === "private_local") {
    require(
      p.external_transmission === "not_used" || p.external_transmission === "consented_private",
      "private_local scope forbids public_only transmission",
    );
  }
}

function requireJudgment(input: unknown): void {
  const j = asRecord(input, "judgment");
  require(
    typeof j.criterion_id === "string" && /^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+$/.test(j.criterion_id),
    "judgment criterion_id is invalid",
  );
  require(
    typeof j.applicability === "string" && APPLICABILITY.has(j.applicability),
    "judgment applicability is invalid",
  );
  require(j.rating === null || (typeof j.rating === "number" && Number.isInteger(j.rating) && j.rating >= 0 && j.rating <= 4), "judgment rating is invalid");
  require(j.rating_scale === 4, "judgment rating_scale must be 4");
  require(typeof j.confidence === "number" && j.confidence >= 0 && j.confidence <= 1, "judgment confidence is invalid");
  const evidenceIds = requireArray(j, "evidence_ids", "judgment");
  for (const id of evidenceIds) {
    require(typeof id === "string" && /^evidence:[a-z0-9._-]+:[a-z0-9._-]+(?::[a-z0-9._-]+)*$/.test(id), "judgment evidence_id is invalid");
  }
  require(typeof j.rationale === "string" && j.rationale.length >= 1 && j.rationale.length <= 1000, "judgment rationale is invalid");
  if (j.applicability === "not_applicable") {
    require(j.rating === null, "not_applicable judgment rating must be null");
  } else {
    require(typeof j.rating === "number" && Number.isInteger(j.rating), "applicable judgment rating is required");
    require(evidenceIds.length >= 1, "applicable judgment requires at least one evidence id");
  }
}