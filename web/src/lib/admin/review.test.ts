import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { getReviewQueue } from "./review";

const schemaDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../schemas/project-ai-analysis");

test("review queue accepts the versioned analysis envelope", async () => {
  const previous = { url: process.env.ASSAY_API_INTERNAL_URL, token: process.env.ASSAY_INTERNAL_ADMIN_TOKEN };
  process.env.ASSAY_API_INTERNAL_URL = "http://api:8080";
  process.env.ASSAY_INTERNAL_ADMIN_TOKEN = "test-token";
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response(JSON.stringify({
    contract: "assay-hosted-api", schema_version: "1.0.0",
    data: [{ evaluation_snapshot_id: "snapshot-1", analysis: JSON.parse(readFileSync(join(schemaDir, "v1.golden.json"), "utf8")) }],
  }), { status: 200, headers: { "content-type": "application/json" } });

  try {
    const result = await getReviewQueue();
    assert.equal(result.state, "available");
    if (result.state === "available") assert.equal(result.items[0]?.analysis.data.judgments.length, 1);
  } finally {
    globalThis.fetch = previousFetch;
    if (previous.url === undefined) delete process.env.ASSAY_API_INTERNAL_URL; else process.env.ASSAY_API_INTERNAL_URL = previous.url;
    if (previous.token === undefined) delete process.env.ASSAY_INTERNAL_ADMIN_TOKEN; else process.env.ASSAY_INTERNAL_ADMIN_TOKEN = previous.token;
  }
});

test("review queue rejects an unwrapped analysis", async () => {
  const previous = { url: process.env.ASSAY_API_INTERNAL_URL, token: process.env.ASSAY_INTERNAL_ADMIN_TOKEN };
  process.env.ASSAY_API_INTERNAL_URL = "http://api:8080";
  process.env.ASSAY_INTERNAL_ADMIN_TOKEN = "test-token";
  const previousFetch = globalThis.fetch;
  const envelope = JSON.parse(readFileSync(join(schemaDir, "v1.golden.json"), "utf8")) as { data: unknown };
  globalThis.fetch = async () => new Response(JSON.stringify({ data: [{ evaluation_snapshot_id: "snapshot-1", analysis: envelope.data }] }), { status: 200 });
  try {
    assert.deepEqual(await getReviewQueue(), { state: "unavailable" });
  } finally {
    globalThis.fetch = previousFetch;
    if (previous.url === undefined) delete process.env.ASSAY_API_INTERNAL_URL; else process.env.ASSAY_API_INTERNAL_URL = previous.url;
    if (previous.token === undefined) delete process.env.ASSAY_INTERNAL_ADMIN_TOKEN; else process.env.ASSAY_INTERNAL_ADMIN_TOKEN = previous.token;
  }
});

test("review queue rejects a raw item array", async () => {
  const previous = { url: process.env.ASSAY_API_INTERNAL_URL, token: process.env.ASSAY_INTERNAL_ADMIN_TOKEN };
  process.env.ASSAY_API_INTERNAL_URL = "http://api:8080";
  process.env.ASSAY_INTERNAL_ADMIN_TOKEN = "test-token";
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response(JSON.stringify([]), { status: 200 });
  try {
    assert.deepEqual(await getReviewQueue(), { state: "unavailable" });
  } finally {
    globalThis.fetch = previousFetch;
    if (previous.url === undefined) delete process.env.ASSAY_API_INTERNAL_URL; else process.env.ASSAY_API_INTERNAL_URL = previous.url;
    if (previous.token === undefined) delete process.env.ASSAY_INTERNAL_ADMIN_TOKEN; else process.env.ASSAY_INTERNAL_ADMIN_TOKEN = previous.token;
  }
});
