import type {
  EvaluatorProvider,
  ProjectEvaluation,
  Visibility,
} from "@/lib/contract/types";

// Engine-profile and result-status presentation from specifications 9 and 12.5.
// Anonymous results start public; authenticated results start as a private
// preview. Provider-unavailable and partial states never expose a retry action.

const VISIBILITY_LABELS: Record<Visibility, string> = {
  public: "Public",
  private_preview: "Private preview",
  private_local: "Local",
};

const PROVIDER_LABELS: Record<EvaluatorProvider, string> = {
  deterministic: "Deterministic",
  openai_api: "OpenAI API",
  codex_cli: "Codex CLI",
  codex_oauth: "Codex OAuth",
};

const STALE_WARNING_CODES = new Set(["stale_snapshot", "snapshot_stale"]);
const PROVIDER_UNAVAILABLE_CODES = new Set([
  "provider_unavailable",
  "ai_provider_unavailable",
  "evaluator_provider_unavailable",
]);

export type ResultBadge = "provisional" | "stale" | "insufficient_evidence";

export interface ResultState {
  visibilityLabel: string;
  profileLabel: string;
  engineLabel: string;
  isAnonymous: boolean;
  releaseGateMet: boolean;
  providerUnavailable: boolean;
  isPartial: boolean;
  badges: ResultBadge[];
}

export function visibilityLabel(visibility: Visibility): string {
  return VISIBILITY_LABELS[visibility];
}

// A public, unauthenticated route may only serve a public result. Private
// previews stay private until explicitly published (OPI-013); authenticated
// access is IAM wiring scope, not this fixture app.
export function isPublicResult(evaluation: ProjectEvaluation): boolean {
  return evaluation.visibility === "public";
}

function hasCode(evaluation: ProjectEvaluation, codes: Set<string>): boolean {
  return [...evaluation.warnings, ...evaluation.limitations].some((d) =>
    codes.has(d.code),
  );
}

export function resultState(evaluation: ProjectEvaluation): ResultState {
  const isAnonymous = evaluation.visibility === "public";
  const releaseGateMet = evaluation.scores.assay_score.value !== null;
  const providerUnavailable = hasCode(evaluation, PROVIDER_UNAVAILABLE_CODES);
  const stale = hasCode(evaluation, STALE_WARNING_CODES);

  const badges: ResultBadge[] = [];
  if (evaluation.provisional) badges.push("provisional");
  if (stale) badges.push("stale");
  if (!releaseGateMet) badges.push("insufficient_evidence");

  return {
    visibilityLabel: VISIBILITY_LABELS[evaluation.visibility],
    profileLabel: isAnonymous ? "Anonymous profile" : "Authenticated profile",
    engineLabel: PROVIDER_LABELS[evaluation.evaluator.provider],
    isAnonymous,
    releaseGateMet,
    providerUnavailable,
    isPartial: evaluation.status === "partial",
    badges,
  };
}
