import assert from "node:assert/strict";
import { test } from "node:test";
import { GET } from "@/app/badge/[...slug]/route";

function request(slug: string[]): Promise<Response> {
  return GET(new Request("http://localhost/badge"), {
    params: Promise.resolve({ slug }),
  });
}

test("serves an SVG badge for a public complete result", async () => {
  const response = await request(["hermes-agent", "hermes"]);
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /image\/svg\+xml/);
});

test("does not serve a badge for an in-flight result", async () => {
  const response = await request(["acme", "in-progress"]);
  assert.equal(response.status, 404);
});

// Regression: a private-preview result must never leak its score through the
// public badge URL (OPI-013).
test("does not serve a badge for a private-preview result", async () => {
  const response = await request(["acme", "degraded"]);
  assert.equal(response.status, 404);
});

test("returns 404 for an unknown project", async () => {
  const response = await request(["nobody", "nothing"]);
  assert.equal(response.status, 404);
});
