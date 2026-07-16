import type { ProjectEvaluation } from "@/lib/contract/types";
import { resultState, type ResultBadge } from "@/lib/state/result-state";

const BADGE_LABELS: Record<ResultBadge, string> = {
  provisional: "Provisional",
  stale: "Stale",
  insufficient_evidence: "Insufficient evidence",
};

export function EngineProfile({ evaluation }: { evaluation: ProjectEvaluation }) {
  const state = resultState(evaluation);
  const { classification, evaluator } = evaluation;

  return (
    <div className="card stack">
      <div className="tag-row">
        <span className="badge">{state.profileLabel}</span>
        <span className="badge">{state.visibilityLabel}</span>
        <span className="badge">{state.engineLabel}</span>
        {state.badges.map((badge) => (
          <span key={badge} className={`badge ${badge}`}>
            {BADGE_LABELS[badge]}
          </span>
        ))}
      </div>

      <dl className="meta">
        <dt>Evaluator profile</dt>
        <dd>{evaluator.profile}</dd>
        <dt>Model</dt>
        <dd>{evaluator.model ?? "n/a (deterministic)"}</dd>
        <dt>Rubric</dt>
        <dd>{evaluator.rubric_version}</dd>
        <dt>Project type</dt>
        <dd>{classification.primary_type ?? "unclassified"}</dd>
        <dt>Maturity</dt>
        <dd>{classification.maturity ?? "unknown"}</dd>
      </dl>

      {classification.tags.length > 0 && (
        <div className="tag-row">
          {classification.tags.map((tag) => (
            <span key={tag} className="badge">
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
