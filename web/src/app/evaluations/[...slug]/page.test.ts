import assert from "node:assert/strict";
import { test } from "node:test";
import EvaluationPage from "@/app/evaluations/[...slug]/page";

function render(slug: string[]): Promise<unknown> {
  return EvaluationPage({ params: Promise.resolve({ slug }) });
}

test("renders a public complete result", async () => {
  await assert.doesNotReject(() => render(["hermes-agent", "hermes"]));
});

test("renders an in-flight progress screen", async () => {
  await assert.doesNotReject(() => render(["acme", "in-progress"]));
});

// A public route must not render a private-preview result until it is
// explicitly published (OPI-013). Authenticated access is IAM wiring scope.
test("does not render a private-preview result on the public route", async () => {
  await assert.rejects(() => render(["acme", "degraded"]));
});

test("does not render an unknown project", async () => {
  await assert.rejects(() => render(["nobody", "nothing"]));
});
