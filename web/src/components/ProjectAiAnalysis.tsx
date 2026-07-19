import type { ProjectAiAnalysis } from "@/lib/contract/types";

export function ProjectAiAnalysisSection({ analysis }: { analysis: ProjectAiAnalysis }) {
  return (
    <section className="ai-analysis" aria-labelledby="ai-analysis-heading">
      <div className="section-head">
        <p className="hero-kicker">bounded interpretation</p>
        <h2 id="ai-analysis-heading">AI analysis</h2>
        <p>
          AI-generated interpretation of bounded public GitHub metadata. This is
          not a project score.
        </p>
      </div>
      <div className="ai-judgments">
        {analysis.judgments.map((judgment) => (
          <article className="ai-judgment" key={judgment.criterion_id}>
            <div className="ai-judgment-header">
              <h3>{judgment.criterion_id}</h3>
              <span className="chip accent">
                {judgment.rating === null
                  ? "Not applicable"
                  : `AI rubric rating, ${judgment.rating}/4`}
              </span>
            </div>
            <dl className="meta">
              <dt>Confidence</dt>
              <dd>{judgment.confidence.toFixed(2)}</dd>
              <dt>Evidence IDs</dt>
              <dd>
                {judgment.evidence_ids.length > 0 ? (
                  <ul className="ai-evidence-list">
                    {judgment.evidence_ids.map((evidenceId) => (
                      <li className="mono" key={evidenceId}>{evidenceId}</li>
                    ))}
                  </ul>
                ) : "None cited"}
              </dd>
            </dl>
            <p className="ai-rationale"><strong>Rationale:</strong> {judgment.rationale}</p>
          </article>
        ))}
      </div>
      <aside className="notice ai-limitations" role="note">
        <strong>Limitations</strong>
        <ul>
          <li>AI-generated interpretation, not a project score.</li>
          <li>Bounded to public GitHub metadata available for this revision.</li>
          <li>Cited evidence supports the interpretation but does not prove every claim.</li>
          {analysis.limitations.map((limitation, index) => <li key={`${index}-${limitation}`}>{limitation}</li>)}
        </ul>
      </aside>
    </section>
  );
}
