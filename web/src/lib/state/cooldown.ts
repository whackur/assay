// Refresh cooldown display from specification 12.3. This computes when a
// resubmission becomes eligible; it is not the abuse quota and grants nothing.

export type EvaluatorProfileKind = "anonymous" | "authenticated";

const COOLDOWN_DAYS: Record<EvaluatorProfileKind, number> = {
  anonymous: 14,
  authenticated: 7,
};

const DAY_MS = 24 * 60 * 60 * 1000;

export interface CooldownStatus {
  profile: EvaluatorProfileKind;
  admitted: boolean;
  nextEligibleAt: string;
  remainingMs: number;
  remainingLabel: string;
}

export function cooldownIntervalMs(profile: EvaluatorProfileKind): number {
  return COOLDOWN_DAYS[profile] * DAY_MS;
}

export function humanizeRemaining(remainingMs: number): string {
  if (remainingMs <= 0) return "now";
  const minutes = Math.ceil(remainingMs / 60_000);
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"}`;
  const hours = Math.ceil(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"}`;
  const days = Math.ceil(hours / 24);
  return `${days} day${days === 1 ? "" : "s"}`;
}

export function cooldownStatus(
  profile: EvaluatorProfileKind,
  lastRunAtIso: string,
  nowIso: string,
): CooldownStatus {
  const lastRun = Date.parse(lastRunAtIso);
  const now = Date.parse(nowIso);
  const nextEligible = lastRun + cooldownIntervalMs(profile);
  const remainingMs = Math.max(0, nextEligible - now);
  return {
    profile,
    admitted: remainingMs === 0,
    nextEligibleAt: new Date(nextEligible).toISOString(),
    remainingMs,
    remainingLabel: humanizeRemaining(remainingMs),
  };
}
