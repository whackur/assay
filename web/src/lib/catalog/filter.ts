// Catalog filtering and ordering. Pure, serializable projections of already-
// compiled evaluations. Missing scores stay unavailable and are never coerced
// to a zero for sorting or filtering.

import type {
  EvaluatorProvider,
  PrimaryType,
} from "@/lib/contract/types";
import type { CatalogEntry } from "./entries";

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