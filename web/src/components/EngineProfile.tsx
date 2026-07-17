import type { ProjectEvaluation } from "@/lib/contract/types";
import { resultState, type ResultBadge } from "@/lib/state/result-state";

const BADGE_LABELS: Record<ResultBadge, string> = {
  provisional: "Provisional",
  stale: "Stale",
  insufficient_evidence: "Insufficient evidence",
};

const BADGE_TONE: Record<ResultBadge, string> = {
  provisional: "chip warn",
  stale: "chip warn",
  insufficient_evidence: "chip",
};

export function EngineProfile({ evaluation }: { evaluation: ProjectEvaluation }) {
  const state = resultState(evaluation);
  const { classification, evaluator } = evaluation;

  return (
    <div className="stack" style={{ marginTop: "var(--space-sm)" }}>
      <div className="chip-row">
        <span className="chip">{state.profileLabel}</span>
        <span className="chip">{state.visibilityLabel}</span>
        <span className="chip accent">{state.engineLabel}</span>
        {state.badges.map((badge) => (
          <span key={badge} className={BADGE_TONE[badge]}>
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
        {classification.tags.length > 0 && (
          <>
            <dt>Tags</dt>
            <dd>{classification.tags.join(", ")}</dd>
          </>
        )}
      </dl>
    </div>
  );
}
