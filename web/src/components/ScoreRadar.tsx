import type { Scores } from "@/lib/contract/types";
import { DIMENSION_LABELS } from "@/lib/state/score-display";
import {
  labelAnchor,
  polygonPoints,
  radarVertex,
  ringPoints,
} from "@/lib/state/radar";

// The score-profile radar: the five rubric dimensions as a pentagon, in
// canonical contract order. One measure, one hue (the copper accent) —
// translucent fill, stroke, and value dots per the dataviz mark specs. The
// aggregate Assay Score and the forward-looking Potential are deliberately not
// axes: this is the project's rubric profile, not a seventh data series.

export const RADAR_DIMENSIONS = [
  "project_substance",
  "originality",
  "engineering_rigor",
  "open_source_readiness",
  "maintenance_health",
] as const;

export type RadarDimension = (typeof RADAR_DIMENSIONS)[number];

// A radar is only drawable when every plotted dimension has a released value;
// a missing score is never coerced to a zero-length spoke.
export function radarValues(scores: Scores): number[] | null {
  const values: number[] = [];
  for (const key of RADAR_DIMENSIONS) {
    const value = scores[key].value;
    if (value === null) return null;
    values.push(value);
  }
  return values;
}

const WIDTH = 460;
const HEIGHT = 348;
const CX = WIDTH / 2;
const CY = 172;
const RADIUS = 108;
const RINGS = [0.25, 0.5, 0.75, 1];

function labelLines(label: string): string[] {
  const words = label.split(" ");
  if (words.length < 2 || label.length <= 12) return [label];
  const mid = Math.ceil(words.length / 2);
  return [words.slice(0, mid).join(" "), words.slice(mid).join(" ")];
}

export function ScoreRadar({ scores }: { scores: Scores }) {
  const values = radarValues(scores);
  if (!values) return null;
  const count = RADAR_DIMENSIONS.length;

  return (
    <div className="radar-wrap">
      <svg
        className="radar"
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        role="img"
        aria-label={`Score profile radar: ${RADAR_DIMENSIONS.map(
          (key, i) => `${DIMENSION_LABELS[key]} ${values[i]} out of 100`,
        ).join(", ")}.`}
      >
        {RINGS.map((level) => (
          <polygon
            key={level}
            className={level === 1 ? "radar-ring radar-ring-outer" : "radar-ring"}
            points={ringPoints(level, count, RADIUS, CX, CY)}
          />
        ))}

        {RADAR_DIMENSIONS.map((key, i) => {
          const end = radarVertex(i, count, 1, RADIUS, CX, CY);
          return (
            <line
              key={key}
              className="radar-axis"
              x1={CX}
              y1={CY}
              x2={end.x}
              y2={end.y}
            />
          );
        })}

        <polygon
          className="radar-area"
          points={polygonPoints(values, RADIUS, CX, CY)}
        />

        {values.map((value, i) => {
          const p = radarVertex(i, count, value / 100, RADIUS, CX, CY);
          return <circle key={i} className="radar-dot" cx={p.x} cy={p.y} r={3.5} />;
        })}

        {RADAR_DIMENSIONS.map((key, i) => {
          const anchor = labelAnchor(i, count);
          const p = radarVertex(i, count, 1.16, RADIUS, CX, CY);
          const lines = labelLines(DIMENSION_LABELS[key]);
          const above = p.y < CY;
          const firstDy = above ? -(lines.length - 1) * 1.1 : 0.35;
          return (
            <text
              key={key}
              className="radar-label"
              x={p.x}
              y={p.y}
              textAnchor={anchor}
            >
              {lines.map((line, li) => (
                <tspan
                  key={li}
                  x={p.x}
                  dy={li === 0 ? `${firstDy}em` : "1.1em"}
                >
                  {line}
                </tspan>
              ))}
              <tspan
                className="radar-label-value"
                x={p.x}
                dy="1.15em"
              >
                {values[i]}
              </tspan>
            </text>
          );
        })}
      </svg>

      {/* The exact values stay available to assistive tech as a list. */}
      <ul className="visually-hidden">
        {RADAR_DIMENSIONS.map((key, i) => (
          <li key={key}>
            {DIMENSION_LABELS[key]}: {values[i]} out of 100
          </li>
        ))}
      </ul>
    </div>
  );
}
