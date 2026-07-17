import Link from "next/link";
import { RECORDS } from "@/lib/api/fixtures";
import { resultState } from "@/lib/state/result-state";
import { ScoreCards } from "@/components/ScoreCards";

// The landing page's demonstration: a condensed, real report rendered from the
// same fixture record the evaluation route serves. Nothing here is marketing
// copy — every value comes out of the versioned report contract.

const SAMPLE_ID = "hermes-agent/hermes";

export function SampleReport() {
  const record = RECORDS[SAMPLE_ID];
  if (!record || record.state !== "complete") return null;
  const { evaluation, evidence } = record;
  const state = resultState(evaluation);
  const fact = evaluation.introduction.factual_statements[0];

  return (
    <figure style={{ margin: 0 }}>
      <div className="sheet">
        <div className="sheet-caption">
          <span>
            specimen · <strong>{SAMPLE_ID}</strong>
          </span>
          <span>
            {state.engineLabel.toLowerCase()} engine · rubric{" "}
            {evaluation.evaluator.rubric_version} · revision{" "}
            {evaluation.project.revision.slice(0, 8)}
          </span>
        </div>
        <div className="sheet-body">
          {fact && <p className="lede">{fact.text}</p>}
          <ScoreCards scores={evaluation.scores} />
          <p className="filter-note">
            {evidence.length} evidence records back this report — files,
            repository features, and the snapshot itself, each graded and
            revision-pinned.
          </p>
        </div>
      </div>
      <figcaption className="sheet-under">
        <span className="mono muted">
          Rendered from the versioned report contract, exactly as the
          evaluation route serves it.
        </span>{" "}
        <Link href={`/evaluations/${SAMPLE_ID}`}>Open the full report</Link>
      </figcaption>
    </figure>
  );
}

// The traceability demonstration: walk one real judgment from score to cited
// evidence record. All values come from the fixture record above.

export function TraceChain() {
  const record = RECORDS[SAMPLE_ID];
  if (!record || record.state !== "complete") return null;
  const { evaluation, evidence } = record;
  const score = evaluation.scores.engineering_rigor;
  const citedId = score.evidence_ids[0];
  const cited = evidence.find((item) => item.id === citedId);

  return (
    <ol className="trace-chain">
      <li className="trace-step">
        <div className="trace-step-label">Judgment</div>
        <div className="trace-step-value">
          Engineering Rigor · {score.value}/100 · {score.version}
        </div>
      </li>
      <li className="trace-step">
        <div className="trace-step-label">Cites</div>
        <div className="trace-step-value">{citedId}</div>
      </li>
      {cited?.provenance && (
        <li className="trace-step">
          <div className="trace-step-label">Evidence record</div>
          <div className="trace-step-value">
            {cited.provenance.source_kind} · grade{" "}
            {cited.grade?.toUpperCase() ?? "—"} · collected{" "}
            {cited.provenance.collected_at.slice(0, 10)} · revision{" "}
            {cited.provenance.repository_revision?.slice(0, 12)}
          </div>
        </li>
      )}
    </ol>
  );
}
