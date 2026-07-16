"use client";

import { useId, useMemo, useState } from "react";
import type { CatalogEntry, CatalogFilter, FilterOptions } from "@/lib/catalog/catalog";
import { filterEntries, recentlyAssayed, topAssays } from "@/lib/catalog/catalog";
import { CatalogList } from "@/components/CatalogList";

// Catalog filter controls and the Recently Assayed / Top Assays lists
// (interview 9). Filtering, ordering, and score-range logic are the pure
// functions in lib/catalog; this component only holds selection state.

const ANY = "";

export function CatalogBrowser({
  entries,
  options,
}: {
  entries: CatalogEntry[];
  options: FilterOptions;
}) {
  const typeId = useId();
  const tagId = useId();
  const engineId = useId();
  const minId = useId();
  const maxId = useId();

  const [primaryType, setPrimaryType] = useState<string>(ANY);
  const [tag, setTag] = useState<string>(ANY);
  const [provider, setProvider] = useState<string>(ANY);
  const [minScore, setMinScore] = useState<string>("0");
  const [maxScore, setMaxScore] = useState<string>("100");

  const filter = useMemo<CatalogFilter>(() => {
    const min = Number(minScore);
    const max = Number(maxScore);
    return {
      primaryType: (primaryType || null) as CatalogFilter["primaryType"],
      tag: tag || null,
      provider: (provider || null) as CatalogFilter["provider"],
      minScore: min > 0 ? min : null,
      maxScore: max < 100 ? max : null,
    };
  }, [primaryType, tag, provider, minScore, maxScore]);

  const filtered = useMemo(() => filterEntries(entries, filter), [entries, filter]);
  const recent = useMemo(() => recentlyAssayed(filtered), [filtered]);
  const top = useMemo(() => topAssays(filtered), [filtered]);

  return (
    <div className="stack">
      <fieldset className="catalog-filters">
        <legend>Filter the catalog</legend>

        <div className="filter-control">
          <label htmlFor={typeId}>Category</label>
          <select id={typeId} value={primaryType} onChange={(e) => setPrimaryType(e.target.value)}>
            <option value={ANY}>All categories</option>
            {options.primaryTypes.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </div>

        <div className="filter-control">
          <label htmlFor={tagId}>Tag</label>
          <select id={tagId} value={tag} onChange={(e) => setTag(e.target.value)}>
            <option value={ANY}>All tags</option>
            {options.tags.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </div>

        <div className="filter-control">
          <label htmlFor={engineId}>Engine</label>
          <select id={engineId} value={provider} onChange={(e) => setProvider(e.target.value)}>
            <option value={ANY}>All engines</option>
            {options.providers.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </div>

        <div className="filter-control">
          <label htmlFor={minId}>Min score</label>
          <input
            id={minId}
            type="number"
            min={0}
            max={100}
            value={minScore}
            onChange={(e) => setMinScore(e.target.value)}
          />
        </div>

        <div className="filter-control">
          <label htmlFor={maxId}>Max score</label>
          <input
            id={maxId}
            type="number"
            min={0}
            max={100}
            value={maxScore}
            onChange={(e) => setMaxScore(e.target.value)}
          />
        </div>
      </fieldset>
      <p className="muted" aria-live="polite">
        A score-range filter lists only released scores; unavailable and
        insufficient results are not treated as a zero.
      </p>

      <section aria-labelledby="recently-assayed">
        <h2 id="recently-assayed">Recently Assayed</h2>
        <CatalogList entries={recent} />
      </section>

      <section aria-labelledby="top-assays">
        <h2 id="top-assays">Top Assays</h2>
        <p className="muted">
          Ranked by released Assay Score with evaluation version, engine,
          confidence, and provisional state.
        </p>
        <CatalogList entries={top} ranked emptyLabel="No released scores match the current filters." />
      </section>
    </div>
  );
}
