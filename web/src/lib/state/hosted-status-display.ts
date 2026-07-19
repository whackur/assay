import type { HostedProjectStatus, HostedRecentSourceStatus } from "@/lib/api/hosted.generated";

type HostedEvaluationStatus = HostedProjectStatus["evaluation_status"];
type HostedScoreStatus = HostedProjectStatus["score_status"];

export function hostedEvaluationLabel(status: HostedEvaluationStatus): string {
  switch (status) {
    case "validated_unpublished":
      return "Validated judgment (not published)";
    case "partial":
      return "Partially evaluated";
    case "unavailable":
      return "Evaluation unavailable";
    default:
      return "Not evaluated";
  }
}

export function hostedScoreLabel(status: HostedScoreStatus): string {
  return status === "unavailable" ? "Unavailable" : "Pending";
}

export function hostedPublicationLabel(status: HostedRecentSourceStatus["evaluation_status"]): string {
  return hostedEvaluationLabel(status);
}
