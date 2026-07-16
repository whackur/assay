import Link from "next/link";
import type { CatalogEntry } from "@/lib/catalog/catalog";
import { scoreSummary } from "@/lib/catalog/catalog";

// A catalog list. The `ranked` variant is the Top Assays list, which
// specification 13 and interview 9 require to expose evaluation version,
// engine, confidence, and provisional state alongside the score.

function CatalogRow({ entry, ranked }: { entry: CatalogEntry; ranked: boolean }) {
  const summary = scoreSummary(entry.score);
  return (
    <li className="catalog-row">
      <div className="catalog-row-main">
        <Link href={`/evaluations/${entry.id}`}>{entry.name}</Link>
        <p className="muted catalog-desc">{entry.description}</p>
        <div className="tag-row">
          {entry.primaryType && <span className="badge">{entry.primaryType}</span>}
          {entry.tags.map((tag) => (
            <span key={tag} className="badge">
              {tag}
            </span>
          ))}
        </div>
      </div>
      <div className="catalog-row-meta">
        <div className="catalog-score" aria-label={summary.released ? `${summary.valueText}` : summary.statusLabel}>
          {summary.released ? summary.valueText : summary.statusLabel}
        </div>
        <div className="muted catalog-score-meta">
          {summary.released && `confidence ${summary.confidencePercent}% (${summary.confidenceBand}) · `}
          {entry.engineLabel}
          {entry.provisional && " · provisional"}
        </div>
        {ranked && (
          <div className="muted catalog-eval-version">
            evaluation {entry.evaluationVersion}
          </div>
        )}
      </div>
    </li>
  );
}

export function CatalogList({
  entries,
  ranked = false,
  emptyLabel = "No projects match the current filters.",
}: {
  entries: CatalogEntry[];
  ranked?: boolean;
  emptyLabel?: string;
}) {
  if (entries.length === 0) {
    return <p className="muted">{emptyLabel}</p>;
  }
  return (
    <ol className="catalog-list">
      {entries.map((entry) => (
        <CatalogRow key={entry.id} entry={entry} ranked={ranked} />
      ))}
    </ol>
  );
}
