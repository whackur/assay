import assert from "node:assert/strict";
import { test } from "node:test";
import { cooldownStatus, humanizeRemaining } from "@/lib/state/cooldown";

test("anonymous cooldown is fourteen days", () => {
  const s = cooldownStatus(
    "anonymous",
    "2026-07-01T00:00:00Z",
    "2026-07-10T00:00:00Z",
  );
  assert.equal(s.admitted, false);
  assert.equal(s.nextEligibleAt, "2026-07-15T00:00:00.000Z");
  assert.equal(s.remainingLabel, "5 days");
});

test("authenticated cooldown is seven days and admits after it elapses", () => {
  const s = cooldownStatus(
    "authenticated",
    "2026-07-01T00:00:00Z",
    "2026-07-09T00:00:00Z",
  );
  assert.equal(s.admitted, true);
  assert.equal(s.remainingMs, 0);
  assert.equal(s.remainingLabel, "now");
});

test("humanize picks the coarsest sensible unit", () => {
  assert.equal(humanizeRemaining(0), "now");
  assert.equal(humanizeRemaining(60_000), "1 minute");
  assert.equal(humanizeRemaining(90 * 60_000), "2 hours");
  assert.equal(humanizeRemaining(48 * 60 * 60_000), "2 days");
});
