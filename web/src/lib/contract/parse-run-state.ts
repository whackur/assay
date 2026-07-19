// Parser for schemas/run-state/v1.json. Validates stage state invariants:
// complete stages carry no reason, partial/unavailable stages require a
// reason, unavailable stages have no snapshot and exhausted retries, and
// purged runs drop every snapshot.

import type { RunState, StageState } from "@/lib/contract/types";
import {
  asRecord,
  isSha256,
  isVersionIdentifier,
  require,
  requireArray,
  requireBoolean,
  requireRecord,
  requireString,
  requireSchemaVersion,
} from "./parse-helpers";

const RUN_STATUS = new Set(["pending", "complete", "partial", "unavailable"]);
const LIFECYCLE = new Set(["active", "deleted", "purged"]);
const STAGES = new Set([
  "source_verification",
  "revision_pinning",
  "file_and_history_analysis",
  "project_type_determination",
  "ci_and_dependency_evidence",
  "similar_project_discovery",
  "ai_rubric_evaluation",
  "score_compilation",
  "result_publication",
]);
const AUDIT_ACTIONS = new Set([
  "rerun_stage",
  "rerun_failed_stages",
  "soft_delete",
  "restore",
  "purge",
]);

export function parseRunState(input: unknown): RunState {
  const value = asRecord(input, "run-state");
  requireSchemaVersion(value, "run-state");
  requireString(value, "run_state_version", "run-state");
  require(isVersionIdentifier(value.run_state_version), "run_state_version is malformed");
  requireString(value, "run_id", "run-state");
  require(isVersionIdentifier(value.run_id), "run_id is malformed");
  require(typeof value.lifecycle === "string" && LIFECYCLE.has(value.lifecycle), "run-state lifecycle is invalid");
  require(typeof value.status === "string" && RUN_STATUS.has(value.status), "run-state status is invalid");
  requireBoolean(value, "ordinary_user_retry_available", "run-state");
  require(value.ordinary_user_retry_available === false, "ordinary_user_retry_available must be false");
  requireRecord(value, "retry_policy", "run-state");
  const policy = value.retry_policy as Record<string, unknown>;
  requireString(policy, "version", "retry_policy");
  require(isVersionIdentifier(policy.version), "retry_policy version is malformed");
  require(
    typeof policy.automatic_retry_budget === "number" && policy.automatic_retry_budget >= 0,
    "retry_policy automatic_retry_budget is invalid",
  );
  const stages = requireArray(value, "stages", "run-state");
  require(stages.length >= 1, "run-state has at least one stage");
  for (const s of stages) requireStage(s);
  if (value.lifecycle === "purged") {
    for (const s of stages) {
      const stage = s as StageState;
      require(stage.result_snapshot === null, "purged run stages drop snapshots");
    }
  }
  const events = requireArray(value, "audit_events", "run-state");
  for (const e of events) requireAuditEvent(e);
  return input as unknown as RunState;
}

function requireStage(input: unknown): void {
  const s = asRecord(input, "stage");
  require(typeof s.stage === "string" && STAGES.has(s.stage), "stage is invalid");
  require(typeof s.status === "string" && RUN_STATUS.has(s.status), "stage status is invalid");
  require(typeof s.attempts === "number" && s.attempts >= 0, "stage attempts is invalid");
  requireBoolean(s, "automatic_retries_exhausted", "stage");
  require(s.reason === null || typeof s.reason === "string", "stage reason is invalid");
  require(s.result_snapshot === null || isSha256(s.result_snapshot), "stage result_snapshot is invalid");
  if (s.status === "complete") {
    require(s.reason === null, "complete stage reason must be null");
    require(s.automatic_retries_exhausted === false, "complete stage must not exhaust retries");
  } else if (s.status === "partial") {
    require(typeof s.reason === "string", "partial stage reason is required");
    require(s.automatic_retries_exhausted === false, "partial stage must not exhaust retries");
  } else if (s.status === "unavailable") {
    require(typeof s.reason === "string", "unavailable stage reason is required");
    require(s.result_snapshot === null, "unavailable stage drops snapshot");
    require(s.automatic_retries_exhausted === true, "unavailable stage exhausts retries");
  } else if (s.status === "pending") {
    require(s.reason === null, "pending stage reason must be null");
    require(s.result_snapshot === null, "pending stage drops snapshot");
    require(s.automatic_retries_exhausted === false, "pending stage must not exhaust retries");
  }
}

function requireAuditEvent(input: unknown): void {
  const e = asRecord(input, "audit event");
  require(typeof e.action === "string" && AUDIT_ACTIONS.has(e.action), "audit action is invalid");
  requireString(e, "run_id", "audit event");
  require(isVersionIdentifier(e.run_id), "audit run_id is malformed");
  requireString(e, "policy_version", "audit event");
  require(isVersionIdentifier(e.policy_version), "audit policy_version is malformed");
  requireString(e, "recorded_at", "audit event");
  require(e.stage === null || (typeof e.stage === "string" && STAGES.has(e.stage)), "audit stage is invalid");
  if (e.action === "rerun_stage") {
    require(typeof e.stage === "string", "rerun_stage audit event requires a stage");
  } else {
    require(e.stage === null, "non-rerun_stage audit event must have null stage");
  }
}