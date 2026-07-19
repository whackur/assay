// Catalog entry shape and the pure projection from a compiled ProjectEvaluation
// into a display entry. Catalog presentation (specification 13, interview 9):
// these are pure, serializable projections that select, filter, and order
// entries for cards and lists. They never derive a score, and a public catalog
// excludes non-public results (OPI-013). Missing scores stay unavailable and
// are never coerced to a zero for sorting or filtering.

import type {
  EvaluatorProvider,
  Maturity,
  PrimaryType,
  ProjectEvaluation,
  Status,
  Visibility,
} from "@/lib/contract/types";
import type { ResultBadge } from "@/lib/state/result-state";
import { resultState } from "@/lib/state/result-state";

export interface CatalogScore {
  status: Status;
  value: number | null;
  confidence: number;
}

export interface CatalogEntry {
  id: string;
  name: string;
  canonicalUrl: string | null;
  description: string;
  primaryType: PrimaryType | null;
  tags: string[];
  maturity: Maturity | null;
  provider: EvaluatorProvider;
  engineLabel: string;
  profile: string;
  evaluationVersion: string;
  provisional: boolean;
  visibility: Visibility;
  assayedAt: string;
  score: CatalogScore;
  badges: ResultBadge[];
  featuredLabel: string | null;
}

function canonicalUrl(evaluation: ProjectEvaluation): string | null {
  const src = evaluation.project.source;
  if (src.kind !== "hosted") return null;
  return `https://${src.provider}.com/${src.namespace}/${src.repository}`;
}

function projectName(evaluation: ProjectEvaluation): string {
  const src = evaluation.project.source;
  return src.kind === "hosted" ? `${src.namespace}/${src.repository}` : src.repository_id;
}

const STATUS_DESCRIPTION: Record<Status, string> = {
  complete: "Introduction available.",
  partial: "Partial evidence; introduction limited.",
  unavailable: "Introduction not available.",
  unsupported: "Introduction not supported for this project.",
  insufficient: "Insufficient evidence for an introduction.",
  pending: "Introduction pending.",
};

function description(evaluation: ProjectEvaluation): string {
  const intro = evaluation.introduction;
  if (intro.status === "complete" && intro.factual_statements.length > 0) {
    return intro.factual_statements[0]!.text;
  }
  return STATUS_DESCRIPTION[intro.status];
}

export function toCatalogEntry(
  evaluation: ProjectEvaluation,
  assayedAt: string,
  featuredLabel: string | null = null,
): CatalogEntry {
  const state = resultState(evaluation);
  const assay = evaluation.scores.assay_score;
  return {
    id: projectName(evaluation),
    name: projectName(evaluation),
    canonicalUrl: canonicalUrl(evaluation),
    description: description(evaluation),
    primaryType: evaluation.classification.primary_type,
    tags: evaluation.classification.tags,
    maturity: evaluation.classification.maturity,
    provider: evaluation.evaluator.provider,
    engineLabel: state.engineLabel,
    profile: evaluation.evaluator.profile,
    evaluationVersion: evaluation.evaluation_version,
    provisional: evaluation.provisional,
    visibility: evaluation.visibility,
    assayedAt,
    score: { status: assay.status, value: assay.value, confidence: assay.confidence },
    badges: state.badges,
    featuredLabel,
  };
}

export function isPublicCatalogEntry(entry: CatalogEntry): boolean {
  return entry.visibility === "public";
}