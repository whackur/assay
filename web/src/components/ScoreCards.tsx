import type { Score, Scores } from "@/lib/contract/types";
import { DIMENSION_LABELS, scoreDisplay } from "@/lib/state/score-display";
import { ScoreNumber } from "@/components/ScoreNumber";

// The score block of the report: the Assay Score as a hero number, the
// dimensions as single-hue magnitude bars with direct labels. An unreleased
// score renders as absent (an em dash and its status), never as a zero-length
// bar pretending to be a measurement.

function DimensionRow({ name, score }: { name: string; score: Score }) {
  const display = scoreDisplay(score);
  return (
    <li className={display.hasValue ? "dim-row" : "dim-row is-absent"}>
      <span className="dim-name">{name}</span>
      {display.hasValue ? (
        <span className="dim-track" aria-hidden="true">
          <span className="dim-fill" style={{ width: `${score.value}%` }} />
        </span>
      ) : (
        <span aria-hidden="true" />
      )}
      <span
        className="dim-value"
        aria-label={
          display.hasValue
            ? `${name}: ${display.valueText} out of 100`
            : `${name}: ${display.statusLabel}`
        }
      >
        {display.valueText}
      </span>
      <span className="dim-meta">
        {display.hasValue
          ? `confidence ${display.confidencePercent}% (${display.confidenceBand})`
          : display.statusLabel}
      </span>
    </li>
  );
}

const DIMENSION_ORDER = [
  "project_substance",
  "originality",
  "engineering_rigor",
  "open_source_readiness",
  "maintenance_health",
] as const;

export function ScoreHero({ score }: { score: Score }) {
  const assay = scoreDisplay(score);
  return (
    <div className="score-hero">
      <span className="score-hero-label">{DIMENSION_LABELS.assay_score}</span>
      <span
        className={assay.hasValue ? "score-hero-value" : "score-hero-value is-absent"}
        aria-label={
          assay.hasValue
            ? `Assay Score ${assay.valueText} out of 100`
            : `Assay Score: ${assay.statusLabel}`
        }
      >
        {score.value !== null ? <ScoreNumber value={score.value} /> : assay.valueText}
      </span>
      {assay.hasValue && <span className="score-hero-denom">/ 100</span>}
      <span className="score-hero-meta">
        {assay.hasValue
          ? `${assay.statusLabel} · confidence ${assay.confidencePercent}% (${assay.confidenceBand})`
          : assay.statusLabel}
      </span>
    </div>
  );
}

export function DimensionBars({ scores }: { scores: Scores }) {
  return (
    <ul className="dim-list">
      {DIMENSION_ORDER.map((key) => (
        <DimensionRow key={key} name={DIMENSION_LABELS[key]} score={scores[key]} />
      ))}
      <DimensionRow name={DIMENSION_LABELS.potential} score={scores.potential} />
    </ul>
  );
}

export function ScoreCards({ scores }: { scores: Scores }) {
  return (
    <div>
      <ScoreHero score={scores.assay_score} />
      <DimensionBars scores={scores} />
    </div>
  );
}
