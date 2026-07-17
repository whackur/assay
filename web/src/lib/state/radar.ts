// Pure geometry for the score-profile radar (pentagon) view. Axis order is the
// canonical dimension order from the report contract, so two renders of the
// same evaluation always produce the same shape. Values are 0–100; a radar is
// only drawn when every plotted dimension has a released value — a missing
// score is never coerced to a zero-length spoke.

export interface RadarPoint {
  x: number;
  y: number;
}

function round2(n: number): number {
  return Math.round(n * 100) / 100;
}

// Vertex i of `count` axes, starting at the top (12 o'clock), clockwise.
export function radarVertex(
  index: number,
  count: number,
  fraction: number,
  radius: number,
  cx: number,
  cy: number,
): RadarPoint {
  if (count < 3) throw new Error("A radar needs at least 3 axes.");
  if (index < 0 || index >= count) throw new Error("Axis index out of range.");
  const angle = -Math.PI / 2 + (index * 2 * Math.PI) / count;
  return {
    x: round2(cx + radius * fraction * Math.cos(angle)),
    y: round2(cy + radius * fraction * Math.sin(angle)),
  };
}

// SVG polygon `points` string for a set of 0–100 values.
export function polygonPoints(
  values: number[],
  radius: number,
  cx: number,
  cy: number,
): string {
  return values
    .map((value, index) => {
      const p = radarVertex(index, values.length, value / 100, radius, cx, cy);
      return `${p.x},${p.y}`;
    })
    .join(" ");
}

// Grid ring at a uniform level (0–1), same axis count as the data polygon.
export function ringPoints(
  level: number,
  count: number,
  radius: number,
  cx: number,
  cy: number,
): string {
  return polygonPoints(new Array<number>(count).fill(level * 100), radius, cx, cy);
}

export type LabelAnchor = "start" | "middle" | "end";

// Text anchor for a perimeter label: top/bottom labels center, right-side
// labels start, left-side labels end.
export function labelAnchor(index: number, count: number): LabelAnchor {
  const angle = -Math.PI / 2 + (index * 2 * Math.PI) / count;
  const cos = Math.cos(angle);
  if (Math.abs(cos) < 0.15) return "middle";
  return cos > 0 ? "start" : "end";
}
