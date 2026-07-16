import type { Score, Status } from "@/lib/contract/types";

// Presentation-only mapping. Values come straight from the compiled contract;
// this never derives, rounds policy, or infers a score.

export type ConfidenceBand = "low" | "medium" | "high";

export interface ScoreDisplay {
  status: Status;
  hasValue: boolean;
  valueText: string;
  statusLabel: string;
  confidencePercent: number | null;
  confidenceBand: ConfidenceBand | null;
}

const STATUS_LABELS: Record<Status, string> = {
  complete: "Scored",
  partial: "Partial evidence",
  unavailable: "Not available",
  unsupported: "Not supported",
  insufficient: "Insufficient evidence",
  pending: "In progress",
};

export function confidenceBand(confidence: number): ConfidenceBand {
  if (confidence >= 0.66) return "high";
  if (confidence >= 0.33) return "medium";
  return "low";
}

export function scoreDisplay(score: Score): ScoreDisplay {
  const hasValue = score.value !== null;
  return {
    status: score.status,
    hasValue,
    valueText: hasValue ? String(score.value) : "—",
    statusLabel: STATUS_LABELS[score.status],
    confidencePercent: hasValue ? Math.round(score.confidence * 100) : null,
    confidenceBand: hasValue ? confidenceBand(score.confidence) : null,
  };
}

export const DIMENSION_LABELS = {
  assay_score: "Assay Score",
  project_substance: "Project Substance",
  originality: "Originality",
  engineering_rigor: "Engineering Rigor",
  open_source_readiness: "Open Source Readiness",
  maintenance_health: "Maintenance Health",
  potential: "Potential",
} as const;
