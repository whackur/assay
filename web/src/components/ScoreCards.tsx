import type { Score, Scores } from "@/lib/contract/types";
import { DIMENSION_LABELS, scoreDisplay } from "@/lib/state/score-display";

function ScoreCard({
  name,
  score,
  headline = false,
}: {
  name: string;
  score: Score;
  headline?: boolean;
}) {
  const display = scoreDisplay(score);
  return (
    <div className={headline ? "score-card headline" : "score-card"}>
      <div className="score-name">{name}</div>
      <div className="score-value" aria-label={display.hasValue ? `${display.valueText} out of 100` : display.statusLabel}>
        {display.valueText}
      </div>
      <div className="score-meta">
        {display.hasValue
          ? `${display.statusLabel} · confidence ${display.confidencePercent}% (${display.confidenceBand})`
          : display.statusLabel}
      </div>
    </div>
  );
}

const DIMENSION_ORDER = [
  "project_substance",
  "originality",
  "engineering_rigor",
  "open_source_readiness",
  "maintenance_health",
] as const;

export function ScoreCards({ scores }: { scores: Scores }) {
  return (
    <div className="score-grid">
      <ScoreCard name={DIMENSION_LABELS.assay_score} score={scores.assay_score} headline />
      {DIMENSION_ORDER.map((key) => (
        <ScoreCard key={key} name={DIMENSION_LABELS[key]} score={scores[key]} />
      ))}
      <ScoreCard name={DIMENSION_LABELS.potential} score={scores.potential} />
    </div>
  );
}
