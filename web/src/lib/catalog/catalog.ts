// Public re-export barrel for catalog presentation. Splits live in ./catalog/*
// by responsibility (entry projection, score summary, filter/sort); this file
// preserves the "@/lib/catalog/catalog" import path every consumer uses.

export {
  isPublicCatalogEntry,
  toCatalogEntry,
  type CatalogEntry,
  type CatalogScore,
} from "./entries";
export { scoreSummary, type ScoreSummary } from "./summary";
export {
  filterEntries,
  filterOptions,
  matchesFilter,
  recentlyAssayed,
  topAssays,
  type CatalogFilter,
  type FilterOptions,
} from "./filter";