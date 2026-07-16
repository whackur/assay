import type { ProjectEvidence } from "@/lib/contract/types";

// Groups evidence for the drill-down explorer. Presentation only.

export interface EvidenceGroup {
  kind: string;
  label: string;
  items: ProjectEvidence[];
}

const KIND_LABELS: Record<string, string> = {
  repository_snapshot: "Repository snapshot",
  file: "Files",
  tracked_file: "Tracked files",
  file_classification: "File classification",
  parent_delta: "Change delta",
  repository_feature: "Repository features",
  history_scope: "History scope",
  reported_ci: "Reported CI",
  deterministic_finding: "Deterministic findings",
  claim_correspondence: "Claim correspondence",
  unavailable: "Requested but unavailable",
};

export function evidenceKind(evidence: ProjectEvidence): string {
  if (evidence.payload?.kind) return evidence.payload.kind;
  if (evidence.requested_kind) return "unavailable";
  return "unknown";
}

export function evidenceLabel(evidence: ProjectEvidence): string {
  const payload = evidence.payload;
  if (payload?.kind === "file" && typeof payload.relative_path === "string") {
    return payload.relative_path;
  }
  if (payload?.kind === "repository_feature" && typeof payload.feature === "string") {
    return payload.feature;
  }
  return evidence.id;
}

export function groupEvidence(list: ProjectEvidence[]): EvidenceGroup[] {
  const byKind = new Map<string, ProjectEvidence[]>();
  for (const evidence of list) {
    const kind = evidenceKind(evidence);
    const bucket = byKind.get(kind) ?? [];
    bucket.push(evidence);
    byKind.set(kind, bucket);
  }
  return [...byKind.entries()].map(([kind, items]) => ({
    kind,
    label: KIND_LABELS[kind] ?? kind,
    items,
  }));
}
