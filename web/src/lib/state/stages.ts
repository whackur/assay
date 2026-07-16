// Named analysis stages from specification 12.5. Progress is expressed as the
// current named stage plus elapsed time, never a fabricated percentage.

export const ANALYSIS_STAGES = [
  "queued",
  "collecting",
  "classifying",
  "comparing",
  "evaluating",
  "compiling",
  "publishing",
] as const;

export type AnalysisStage = (typeof ANALYSIS_STAGES)[number];

const STAGE_LABELS: Record<AnalysisStage, string> = {
  queued: "Queued",
  collecting: "Collecting public facts",
  classifying: "Classifying project",
  comparing: "Discovering similar projects",
  evaluating: "AI evidence-rubric evaluation",
  compiling: "Compiling versioned scores",
  publishing: "Publishing result",
};

export function stageLabel(stage: AnalysisStage): string {
  return STAGE_LABELS[stage];
}

export function stageIndex(stage: AnalysisStage): number {
  return ANALYSIS_STAGES.indexOf(stage);
}

export function isStageComplete(stage: AnalysisStage, current: AnalysisStage): boolean {
  return stageIndex(stage) < stageIndex(current);
}

export function formatElapsed(elapsedMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (hours > 0) return `${hours}:${pad(minutes)}:${pad(seconds)}`;
  return `${minutes}:${pad(seconds)}`;
}
