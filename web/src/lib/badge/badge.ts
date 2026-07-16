import type { ProjectEvaluation } from "@/lib/contract/types";
import { resultState } from "@/lib/state/result-state";

// README SVG badge (WEB-003). The badge is a pure function of a compiled
// evaluation: it never derives a score, only presents the released value and
// the provisional/stale/insufficient_evidence state. Rendering is
// self-contained (no external font or resource) and escapes every input so a
// hostile field cannot inject markup into the SVG.

export type BadgeTone = "ok" | "warn" | "neutral";

export interface BadgeInput {
  label: string;
  message: string;
  tone: BadgeTone;
}

// Fixed geometry keeps the golden output byte-stable regardless of the runtime
// font: an approximate per-character advance rather than measured metrics.
const CHAR_WIDTH = 7;
const CELL_PADDING = 6;
const HEIGHT = 20;
const FONT_FAMILY = "Verdana,DejaVu Sans,Geneva,sans-serif";
const FONT_SIZE = 11;

const LABEL_BG = "#24292f";
const TONE_BG: Record<BadgeTone, string> = {
  ok: "#1a7f45",
  warn: "#8a5a00",
  neutral: "#57606a",
};

export function badgeInput(evaluation: ProjectEvaluation): BadgeInput {
  const state = resultState(evaluation);
  const label = `assay: ${state.engineLabel}`;
  const value = evaluation.scores.assay_score.value;

  if (value === null) {
    return { label, message: "insufficient evidence", tone: "neutral" };
  }

  const flag = evaluation.provisional
    ? "provisional"
    : state.badges.includes("stale")
      ? "stale"
      : null;
  return {
    label,
    message: flag ? `${value}/100 ${flag}` : `${value}/100`,
    tone: flag ? "warn" : "ok",
  };
}

function escapeXml(text: string): string {
  return text.replace(/[&<>"']/g, (ch) => {
    switch (ch) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      default:
        return "&#39;";
    }
  });
}

function cellWidth(text: string): number {
  return [...text].length * CHAR_WIDTH + CELL_PADDING * 2;
}

export function renderBadge(input: BadgeInput): string {
  const leftWidth = cellWidth(input.label);
  const rightWidth = cellWidth(input.message);
  const total = leftWidth + rightWidth;
  const leftCenter = leftWidth / 2;
  const rightCenter = leftWidth + rightWidth / 2;
  const aria = `${input.label} ${input.message}`;

  return [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${total}" height="${HEIGHT}" role="img" aria-label="${escapeXml(aria)}">`,
    `<title>${escapeXml(aria)}</title>`,
    `<clipPath id="r"><rect width="${total}" height="${HEIGHT}" rx="3" fill="#fff"/></clipPath>`,
    `<g clip-path="url(#r)">`,
    `<rect width="${leftWidth}" height="${HEIGHT}" fill="${LABEL_BG}"/>`,
    `<rect x="${leftWidth}" width="${rightWidth}" height="${HEIGHT}" fill="${TONE_BG[input.tone]}"/>`,
    `</g>`,
    `<g fill="#ffffff" font-family="${FONT_FAMILY}" font-size="${FONT_SIZE}" text-anchor="middle">`,
    `<text x="${leftCenter}" y="14">${escapeXml(input.label)}</text>`,
    `<text x="${rightCenter}" y="14">${escapeXml(input.message)}</text>`,
    `</g>`,
    `</svg>`,
  ].join("");
}

export function badgeSvg(evaluation: ProjectEvaluation): string {
  return renderBadge(badgeInput(evaluation));
}
