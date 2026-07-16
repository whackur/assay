import { RECORDS } from "@/lib/api/fixtures";
import type { CatalogEntry } from "@/lib/catalog/catalog";
import { isPublicCatalogEntry, toCatalogEntry } from "@/lib/catalog/catalog";

// Catalog listing metadata. `assayed_at` orders Recently Assayed; `featured`
// labels an editorially featured home card, which specification 13 requires to
// be independent of the score and visibly labeled. Listings reference complete
// evaluation records; a private-preview record is filtered out of the public
// catalog rather than hidden by omission.

export interface CatalogListing {
  id: string;
  assayed_at: string;
  featured?: string;
}

export const CATALOG_LISTINGS: CatalogListing[] = [
  { id: "hermes-agent/hermes", assayed_at: "2026-07-16T09:00:00Z", featured: "Hermes Agent" },
  { id: "openclaw/openclaw", assayed_at: "2026-07-15T11:00:00Z", featured: "OpenClaw" },
  { id: "example-org/sample-project", assayed_at: "2026-07-14T08:00:00Z" },
  { id: "acme/recently-analyzed", assayed_at: "2026-07-13T08:00:00Z" },
  { id: "example-org/early-prototype", assayed_at: "2026-07-12T08:00:00Z" },
  { id: "acme/degraded", assayed_at: "2026-07-11T08:00:00Z" },
];

function entryFor(listing: CatalogListing): CatalogEntry | null {
  const record = RECORDS[listing.id];
  if (!record || record.state !== "complete") return null;
  return toCatalogEntry(record.evaluation, listing.assayed_at, listing.featured ?? null);
}

export function publicCatalogEntries(): CatalogEntry[] {
  return CATALOG_LISTINGS.map(entryFor)
    .filter((entry): entry is CatalogEntry => entry !== null)
    .filter(isPublicCatalogEntry);
}

export function featuredEntries(): CatalogEntry[] {
  return publicCatalogEntries().filter((entry) => entry.featuredLabel !== null);
}
