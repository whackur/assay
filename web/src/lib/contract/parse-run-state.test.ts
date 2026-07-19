import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseRunState } from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real run-state golden fixture", () => {
  const state = parseRunState(golden("run-state-v1.json"));
  assert.equal(state.run_id, "run-0000000042");
  assert.equal(state.lifecycle, "active");
  assert.equal(state.status, "partial");
  assert.equal(state.ordinary_user_retry_available, false);
  assert.equal(state.stages.length, 9);
  assert.equal(state.stages[5]?.status, "unavailable");
  assert.equal(state.stages[5]?.automatic_retries_exhausted, true);
  assert.equal(state.audit_events[0]?.action, "soft_delete");
});

test("rejects a run-state with ordinary_user_retry_available true", () => {
  const fixture = golden("run-state-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseRunState({ ...fixture, ordinary_user_retry_available: true }),
    ContractError,
  );
});

test("rejects a complete stage with a reason", () => {
  const fixture = golden("run-state-v1.json") as Record<string, unknown>;
  const stages = [...(fixture.stages as object[])];
  stages[0] = { ...stages[0], reason: "should_not_be_here" };
  assert.throws(
    () => parseRunState({ ...fixture, stages }),
    ContractError,
  );
});

test("rejects an unavailable stage without exhausted retries", () => {
  const fixture = golden("run-state-v1.json") as Record<string, unknown>;
  const stages = [...(fixture.stages as object[])];
  stages[5] = { ...stages[5], automatic_retries_exhausted: false };
  assert.throws(
    () => parseRunState({ ...fixture, stages }),
    ContractError,
  );
});

test("rejects a rerun_stage audit event with null stage", () => {
  const fixture = golden("run-state-v1.json") as Record<string, unknown>;
  const events = [...(fixture.audit_events as object[])];
  events[0] = { ...events[0], action: "rerun_stage", stage: null };
  assert.throws(
    () => parseRunState({ ...fixture, audit_events: events }),
    ContractError,
  );
});

test("rejects a purged run with a retained snapshot", () => {
  const fixture = golden("run-state-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseRunState({ ...fixture, lifecycle: "purged" }),
    ContractError,
  );
});