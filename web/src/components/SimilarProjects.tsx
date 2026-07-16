import type {
  CompactCandidate,
  DetailedCandidate,
  ProjectComparison,
  Similarity,
} from "@/lib/contract/types";

// Renders the project-comparison/v1 contract on the detail page only
// (specification 13). Similarity is never a quality signal and never implies
// misconduct; popularity is context only; an unavailable facet shows as absent,
// never a zero similarity.

function candidateName(source: DetailedCandidate["candidate"]["source"]): string {
  return `${source.namespace}/${source.repository}`;
}

function similarityText(similarity: Similarity): string {
  if (similarity.status !== "complete" || similarity.value === null) {
    return `not available (${similarity.status})`;
  }
  return `${Math.round(similarity.value * 100)}% similar`;
}

function facetLabel(token: string): string {
  return token.replace(/_/g, " ");
}

function DetailedCandidateCard({ candidate }: { candidate: DetailedCandidate }) {
  return (
    <li className="card stack">
      <h3 className="featured-name">{candidateName(candidate.candidate.source)}</h3>
      <p>
        {similarityText(candidate.overall_similarity)} · confidence{" "}
        {Math.round(candidate.confidence * 100)}%
      </p>

      <div>
        <span className="muted">Selected because of </span>
        {candidate.selection_reasons.map(facetLabel).join(", ")}
      </div>

      <dl className="meta">
        {candidate.facets.map((facet) => (
          <div key={facet.facet} className="facet-row">
            <dt>{facetLabel(facet.facet)}</dt>
            <dd>{similarityText(facet)}</dd>
          </div>
        ))}
      </dl>

      {(candidate.differentiators.seed_only.length > 0 ||
        candidate.differentiators.candidate_only.length > 0) && (
        <div className="stack">
          {candidate.differentiators.seed_only.length > 0 && (
            <p className="muted">
              This project only:{" "}
              {candidate.differentiators.seed_only.map((d) => facetLabel(d.token)).join(", ")}
            </p>
          )}
          {candidate.differentiators.candidate_only.length > 0 && (
            <p className="muted">
              Candidate only:{" "}
              {candidate.differentiators.candidate_only.map((d) => facetLabel(d.token)).join(", ")}
            </p>
          )}
        </div>
      )}

      <p className="muted catalog-score-meta">
        Adoption context: {candidate.popularity.stars === null ? "unknown" : `${candidate.popularity.stars} stars`}
      </p>
    </li>
  );
}

function CompactCandidateRow({ candidate }: { candidate: CompactCandidate }) {
  return (
    <li className="catalog-row">
      <span>{candidateName(candidate.candidate.source)}</span>
      <span className="muted">
        {similarityText(candidate.overall_similarity)} · confidence{" "}
        {Math.round(candidate.confidence * 100)}%
      </span>
    </li>
  );
}

export function SimilarProjects({ comparison }: { comparison: ProjectComparison }) {
  return (
    <section aria-labelledby="similar-projects">
      <h2 id="similar-projects">Similar projects</h2>
      <p className="muted">
        A one-depth functional cohort. Similarity measures comparability, not
        quality, and never implies misconduct. Popularity is context only.
      </p>

      {comparison.detailed_candidates.length === 0 ? (
        <p className="muted">No sufficiently similar projects were found.</p>
      ) : (
        <ol className="catalog-list">
          {comparison.detailed_candidates.map((candidate) => (
            <DetailedCandidateCard
              key={candidateName(candidate.candidate.source)}
              candidate={candidate}
            />
          ))}
        </ol>
      )}

      {comparison.additional_candidates.length > 0 && (
        <details className="card">
          <summary>Additional candidates ({comparison.additional_candidates.length})</summary>
          <ul className="catalog-list">
            {comparison.additional_candidates.map((candidate) => (
              <CompactCandidateRow
                key={candidateName(candidate.candidate.source)}
                candidate={candidate}
              />
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
