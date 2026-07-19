// TypeScript mirror of schemas/ai-judgment/v1.json. A bounded qualitative
// rubric result. It is not a published score and cannot override the
// deterministic project score compiler.

import type { Status } from "./common";

export type JudgmentStatus = Status;

export type EvidenceScope = "public_only" | "private_local";

export type ExternalTransmission =
  | "not_used"
  | "public_only"
  | "consented_private";

export type JudgmentApplicability =
  | "applicable"
  | "partially_applicable"
  | "not_applicable";

export interface JudgmentPrivacy {
  evidence_scope: EvidenceScope;
  external_transmission: ExternalTransmission;
}

export interface Judgment {
  criterion_id: string;
  applicability: JudgmentApplicability;
  rating: number | null;
  rating_scale: 4;
  confidence: number;
  evidence_ids: string[];
  rationale: string;
}

export interface AiJudgment {
  schema_version: string;
  evaluation_version: string;
  rubric_version: string;
  status: JudgmentStatus;
  evidence_bundle_hash: string;
  privacy: JudgmentPrivacy;
  judgments: Judgment[];
}