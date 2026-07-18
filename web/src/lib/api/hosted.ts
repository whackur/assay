import "server-only";

import {
  isHostedProjectStatusResponse,
  isHostedRecentSourcesResponse,
} from "./hosted.generated";
import type {
  HostedProjectStatus,
  HostedProjectStatusResponse,
  HostedRecentSourceStatus,
  HostedRecentSourcesResponse,
} from "./hosted.generated";

export type { HostedProjectStatus, HostedRecentSourceStatus } from "./hosted.generated";

export type HostedResult<T> =
  | { state: "available"; data: T }
  | { state: "unavailable"; reason: "api_unavailable" | "not_found" };

function apiBase(): string | null {
  const value = process.env.ASSAY_API_INTERNAL_URL?.trim();
  if (!value) return null;
  try {
    const parsed = new URL(value);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
    return value.replace(/\/$/, "");
  } catch {
    return null;
  }
}

async function get<T>(
  path: string,
  validate: (value: unknown) => value is { data: T },
): Promise<HostedResult<T>> {
  const base = apiBase();
  if (!base) return { state: "unavailable", reason: "api_unavailable" };
  try {
    const response = await fetch(`${base}${path}`, {
      cache: "no-store",
      signal: AbortSignal.timeout(4_000),
    });
    if (response.status === 404) return { state: "unavailable", reason: "not_found" };
    if (!response.ok) return { state: "unavailable", reason: "api_unavailable" };
    const envelope: unknown = await response.json();
    if (!validate(envelope)) {
      return { state: "unavailable", reason: "api_unavailable" };
    }
    return { state: "available", data: envelope.data };
  } catch {
    return { state: "unavailable", reason: "api_unavailable" };
  }
}

export function getHostedRecentSources(): Promise<HostedResult<HostedRecentSourceStatus[]>> {
  return get<HostedRecentSourcesResponse["data"]>(
    "/internal/hosted/recent-sources",
    isHostedRecentSourcesResponse,
  );
}

export function getHostedProject(
  owner: string,
  repository: string,
): Promise<HostedResult<HostedProjectStatus>> {
  return get<HostedProjectStatusResponse["data"]>(
    `/internal/hosted/projects/github/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}`,
    isHostedProjectStatusResponse,
  );
}

export function internalApiBase(): string | null {
  return apiBase();
}
