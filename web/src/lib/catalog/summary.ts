// Score summary presentation for a catalog entry. Presentation only; values
// come straight from the compiled contract, never derived or rounded here.

import type { Status } from "@/lib/contract/types";
import type { ConfidenceBand } from "@/lib/state/score-display";
import { confidenceBand } from "@/lib/state/score-display";
import type { CatalogScore } from "./entries";

const SCORE_STATUS_LABELS: Record<Status, string> = {
  complete: "Scored",
  partial: "Partial",
  unavailable: "Not available",
  unsupported: "Not supported",
  insufficient: "Insufficient evidence",
  pending: "In progress",
};

export interface ScoreSummary {
  released: boolean;
  valueText: string;
  statusLabel: string;
  confidencePercent: number | null;
  confidenceBand: ConfidenceBand | null;
}

export function scoreSummary(score: CatalogScore): ScoreSummary {
  const released = score.value !== null;
  return {
    released,
    valueText: released ? `${score.value}/100` : "—",
    statusLabel: SCORE_STATUS_LABELS[score.status],
    confidencePercent: released ? Math.round(score.confidence * 100) : null,
    confidenceBand: released ? confidenceBand(score.confidence) : null,
  };
}