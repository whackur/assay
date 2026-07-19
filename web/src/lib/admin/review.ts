import type { ProjectAiAnalysisEnvelope } from "@/lib/contract/types";
import { parseProjectAiAnalysis } from "@/lib/contract/parse-project-ai-analysis";

export interface ReviewQueueItem { evaluation_snapshot_id: string; analysis: ProjectAiAnalysisEnvelope }
export type ReviewQueueResult = { state: "available"; items: ReviewQueueItem[] } | { state: "unavailable" };

function serviceToken(): string | null {
  const value = process.env.ASSAY_INTERNAL_ADMIN_TOKEN?.trim();
  return value && value.length <= 256 ? value : null;
}

function internalApiBase(): string | null {
  const value = process.env.ASSAY_API_INTERNAL_URL?.trim();
  if (!value) return null;
  try { const parsed = new URL(value); return parsed.protocol === "http:" || parsed.protocol === "https:" ? value.replace(/\/$/, "") : null; } catch { return null; }
}

function isReviewItem(value: unknown): value is ReviewQueueItem {
  if (!value || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  if (typeof item.evaluation_snapshot_id !== "string" || item.evaluation_snapshot_id.length === 0) return false;
  try { parseProjectAiAnalysis(item.analysis); return true; } catch { return false; }
}

export async function getReviewQueue(): Promise<ReviewQueueResult> {
  const base = internalApiBase(); const token = serviceToken();
  if (!base || !token) return { state: "unavailable" };
  try {
    const response = await fetch(`${base}/internal/admin/hosted/ai-analysis/review-queue`, {
      headers: { authorization: `Bearer ${token}` }, cache: "no-store", signal: AbortSignal.timeout(4_000),
    });
    if (!response.ok) return { state: "unavailable" };
    const value: unknown = await response.json();
    const data = value && typeof value === "object" && Object.keys(value).length === 3 && "contract" in value && "schema_version" in value && "data" in value
      && (value as { contract?: unknown }).contract === "assay-hosted-api"
      && (value as { schema_version?: unknown }).schema_version === "1.0.0"
      ? (value as { data: unknown }).data : null;
    return Array.isArray(data) && data.length <= 50 && data.every(isReviewItem)
      ? { state: "available", items: data } : { state: "unavailable" };
  } catch { return { state: "unavailable" }; }
}

export async function approveReviewItem(id: string, principal: { issuer: string; subject: string; displayName: string }): Promise<number | null> {
  const base = internalApiBase(); const token = serviceToken();
  if (!base || !token) return null;
  try {
    const response = await fetch(`${base}/internal/admin/hosted/ai-analysis/approve`, {
      method: "POST", cache: "no-store", signal: AbortSignal.timeout(4_000),
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json",
        "x-assay-identity-issuer": principal.issuer, "x-assay-identity-subject": principal.subject,
        "x-assay-identity-display-name": principal.displayName },
      body: JSON.stringify({ evaluation_snapshot_id: id }),
    });
    return response.status;
  } catch { return null; }
}
