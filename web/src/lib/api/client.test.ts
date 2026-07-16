import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureApi } from "@/lib/api/client";

test("invalid input yields an invalid outcome", async () => {
  const outcome = await fixtureApi.submit("https://gitlab.com/o/r");
  assert.equal(outcome.kind, "invalid");
});

test("a known complete result without cooldown navigates to the cache", async () => {
  const outcome = await fixtureApi.submit(
    "https://github.com/example-org/sample-project",
    "2026-07-16T00:00:00Z",
  );
  assert.equal(outcome.kind, "cached");
  assert.equal(outcome.kind === "cached" && outcome.id, "example-org/sample-project");
});

test("a recent run inside the refresh cooldown yields a cooldown outcome", async () => {
  const outcome = await fixtureApi.submit(
    "github.com/acme/recently-analyzed",
    "2026-07-16T00:00:00Z",
  );
  assert.equal(outcome.kind, "cooldown");
  if (outcome.kind !== "cooldown") return;
  assert.equal(outcome.id, "acme/recently-analyzed");
  assert.equal(outcome.cooldown.admitted, false);
  assert.equal(outcome.cooldown.profile, "anonymous");
  assert.equal(outcome.cooldown.nextEligibleAt, "2026-07-28T00:00:00.000Z");
  assert.equal(outcome.cooldown.remainingLabel, "12 days");
});

test("the same target after the cooldown elapses navigates to the cache", async () => {
  const outcome = await fixtureApi.submit(
    "github.com/acme/recently-analyzed",
    "2026-08-01T00:00:00Z",
  );
  assert.equal(outcome.kind, "cached");
});

test("an unknown repository is admitted as a new job", async () => {
  const outcome = await fixtureApi.submit("github.com/brand/new-thing");
  assert.equal(outcome.kind, "admitted");
});
