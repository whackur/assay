import Link from "next/link";
import type { CatalogEntry } from "@/lib/catalog/catalog";
import { scoreSummary } from "@/lib/catalog/catalog";

// Independent featured introduction card (specification 13, interview 9).
// Featuring is editorial and labeled; it does not reflect or influence the
// score. Comparisons live on the detail page, never on this card.

const BADGE_LABELS: Record<CatalogEntry["badges"][number], string> = {
  provisional: "Provisional",
  stale: "Stale",
  insufficient_evidence: "Insufficient evidence",
};

export function FeaturedCard({ entry }: { entry: CatalogEntry }) {
  const summary = scoreSummary(entry.score);
  return (
    <article className="card stack featured-card" aria-labelledby={`featured-${entry.id}`}>
      <div className="tag-row">
        <span className="badge featured-tag">Featured</span>
        {entry.featuredLabel && <span className="badge">{entry.featuredLabel}</span>}
      </div>

      <h3 id={`featured-${entry.id}`} className="featured-name">
        <Link href={`/evaluations/${entry.id}`}>{entry.name}</Link>
      </h3>
      <p>{entry.description}</p>

      <dl className="meta">
        <dt>Type</dt>
        <dd>{entry.primaryType ?? "unclassified"}</dd>
        <dt>Maturity</dt>
        <dd>{entry.maturity ?? "unknown"}</dd>
        <dt>Engine</dt>
        <dd>{entry.engineLabel}</dd>
        <dt>Assay Score</dt>
        <dd>
          {summary.released
            ? `${summary.valueText} · confidence ${summary.confidencePercent}% (${summary.confidenceBand})`
            : summary.statusLabel}
        </dd>
      </dl>

      {entry.badges.length > 0 && (
        <div className="tag-row">
          {entry.badges.map((badge) => (
            <span key={badge} className={`badge ${badge}`}>
              {BADGE_LABELS[badge]}
            </span>
          ))}
        </div>
      )}

      <p>
        <Link href={`/evaluations/${entry.id}`}>Read the full evaluation</Link>
      </p>
    </article>
  );
}
