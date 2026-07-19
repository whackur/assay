// TypeScript mirror of schemas/run-state/v1.json. The stage state of one
// project analysis run. A partial stage failure preserves completed stages;
// only failed stages carry partial or unavailable plus a reason. Bounded
// automatic retries are policy data, ordinary users cannot retry, and
// administrator recovery actions are audited without secrets.

export type RunLifecycle = "active" | "deleted" | "purged";

export type RunStatus = "pending" | "complete" | "partial" | "unavailable";

export type RunStage =
  | "source_verification"
  | "revision_pinning"
  | "file_and_history_analysis"
  | "project_type_determination"
  | "ci_and_dependency_evidence"
  | "similar_project_discovery"
  | "ai_rubric_evaluation"
  | "score_compilation"
  | "result_publication";

export type AuditAction =
  | "rerun_stage"
  | "rerun_failed_stages"
  | "soft_delete"
  | "restore"
  | "purge";

export interface RetryPolicy {
  version: string;
  automatic_retry_budget: number;
}

export interface StageState {
  stage: RunStage;
  status: RunStatus;
  attempts: number;
  automatic_retries_exhausted: boolean;
  reason: string | null;
  result_snapshot: string | null;
}

export interface AuditEvent {
  action: AuditAction;
  run_id: string;
  stage: RunStage | null;
  policy_version: string;
  recorded_at: string;
}

export interface RunState {
  schema_version: string;
  run_state_version: string;
  run_id: string;
  lifecycle: RunLifecycle;
  status: RunStatus;
  ordinary_user_retry_available: false;
  retry_policy: RetryPolicy;
  stages: StageState[];
  audit_events: AuditEvent[];
}