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
import type { ConfidenceBand } from "@/lib/state/score-display";
import { confidenceBand } from "@/lib/state/score-display";

// Catalog presentation (specification 13, interview 9). These are pure,
// serializable projections of already-compiled evaluations: they select,
// filter, and order entries for cards and lists. They never derive a score,
// and a public catalog excludes non-public results (OPI-013). Missing scores
// stay unavailable and are never coerced to a zero for sorting or filtering.

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

export interface CatalogFilter {
  primaryType?: PrimaryType | null;
  tag?: string | null;
  provider?: EvaluatorProvider | null;
  minScore?: number | null;
  maxScore?: number | null;
}

export interface FilterOptions {
  primaryTypes: PrimaryType[];
  tags: string[];
  providers: EvaluatorProvider[];
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

const SCORE_STATUS_LABELS: Record<Status, string> = {
  complete: "Scored",
  partial: "Partial",
  unavailable: "Not available",
  unsupported: "Not supported",
  insufficient: "Insufficient evidence",
  pending: "In progress",
};

export interface ScoreSummary {
  released: boolean;
  valueText: string;
  statusLabel: string;
  confidencePercent: number | null;
  confidenceBand: ConfidenceBand | null;
}

export function scoreSummary(score: CatalogScore): ScoreSummary {
  const released = score.value !== null;
  return {
    released,
    valueText: released ? `${score.value}/100` : "—",
    statusLabel: SCORE_STATUS_LABELS[score.status],
    confidencePercent: released ? Math.round(score.confidence * 100) : null,
    confidenceBand: released ? confidenceBand(score.confidence) : null,
  };
}

export function matchesFilter(entry: CatalogEntry, filter: CatalogFilter): boolean {
  if (filter.primaryType && entry.primaryType !== filter.primaryType) return false;
  if (filter.tag && !entry.tags.includes(filter.tag)) return false;
  if (filter.provider && entry.provider !== filter.provider) return false;

  const min = filter.minScore ?? null;
  const max = filter.maxScore ?? null;
  if (min !== null || max !== null) {
    // A score-range filter is a query over released scores. Entries without a
    // released value are excluded from the range rather than treated as zero.
    if (entry.score.value === null) return false;
    if (min !== null && entry.score.value < min) return false;
    if (max !== null && entry.score.value > max) return false;
  }
  return true;
}

export function filterEntries(entries: CatalogEntry[], filter: CatalogFilter): CatalogEntry[] {
  return entries.filter((entry) => matchesFilter(entry, filter));
}

export function recentlyAssayed(entries: CatalogEntry[]): CatalogEntry[] {
  return [...entries].sort((a, b) => {
    const byTime = Date.parse(b.assayedAt) - Date.parse(a.assayedAt);
    return byTime !== 0 ? byTime : a.name.localeCompare(b.name);
  });
}

// Top Assays ranks only released scores. Provisional and low-confidence entries
// remain, labeled by their badges and confidence, but unavailable scores are
// omitted rather than ranked as a zero.
export function topAssays(entries: CatalogEntry[]): CatalogEntry[] {
  return entries
    .filter((entry) => entry.score.value !== null)
    .sort((a, b) => {
      const byScore = b.score.value! - a.score.value!;
      if (byScore !== 0) return byScore;
      const byConfidence = b.score.confidence - a.score.confidence;
      return byConfidence !== 0 ? byConfidence : a.name.localeCompare(b.name);
    });
}

export function filterOptions(entries: CatalogEntry[]): FilterOptions {
  const primaryTypes = new Set<PrimaryType>();
  const tags = new Set<string>();
  const providers = new Set<EvaluatorProvider>();
  for (const entry of entries) {
    if (entry.primaryType) primaryTypes.add(entry.primaryType);
    for (const tag of entry.tags) tags.add(tag);
    providers.add(entry.provider);
  }
  return {
    primaryTypes: [...primaryTypes].sort(),
    tags: [...tags].sort(),
    providers: [...providers].sort(),
  };
}
