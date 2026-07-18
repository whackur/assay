import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  HOSTED_API_CONTRACT,
  HOSTED_API_SCHEMA_SHA256,
  HOSTED_API_SCHEMA_VERSION,
  isHostedProjectStatusResponse,
  isHostedRecentSourcesResponse,
  isHostedSubmissionResponse,
} from "./hosted.generated";

test("generated hosted types bind the exact schema source hash", async () => {
  const schema = await readFile(
    path.resolve(process.cwd(), "../schemas/hosted-api/1.0.0.json"),
  );
  const hash = createHash("sha256").update(schema).digest("hex");
  assert.equal(HOSTED_API_SCHEMA_SHA256, hash);
  assert.equal(HOSTED_API_CONTRACT, "assay-hosted-api");
  assert.equal(HOSTED_API_SCHEMA_VERSION, "1.0.0");
});

const envelope = (data: unknown) => ({
  contract: HOSTED_API_CONTRACT,
  schema_version: HOSTED_API_SCHEMA_VERSION,
  data,
});

test("generated runtime validators accept every hosted response variant and reject drift", () => {
  for (const [state, admission, retry_after_seconds] of [
    ["queued", "admitted", null],
    ["collecting", "joined_active", null],
    ["complete", "cooldown", 60],
  ] as const) {
    assert.equal(isHostedSubmissionResponse(envelope({
      request_id: "00000000-0000-4000-8000-000000000000",
      owner: "whackur", repository: "assay",
      canonical_url: "https://github.com/whackur/assay",
      state, admission, retry_after_seconds,
    })), true);
  }

  const project = {
    request_id: "00000000-0000-4000-8000-000000000000",
    owner: "whackur", repository: "assay",
    canonical_url: "https://github.com/whackur/assay",
    request_state: "complete", job_stage: "evaluating", job_state: "complete",
    last_error_code: null, provider_repository_id: "42", default_branch: "main",
    head_sha: "0123456789abcdef0123456789abcdef01234567", description: null,
    stars: 12, evaluation_status: "validated_unpublished", score_status: "unavailable",
    next_attempt_at: "2026-07-18T12:00:00Z", updated_at: "2026-07-18T12:00:00Z",
  };
  assert.equal(isHostedProjectStatusResponse(envelope(project)), true);
  for (const request_state of ["queued", "collecting", "partial", "complete", "unavailable"] as const) {
    assert.equal(isHostedProjectStatusResponse(envelope({ ...project, request_state })), true);
  }
  for (const job_stage of ["canonicalizing", "collecting", "evaluating", "publishing"] as const) {
    assert.equal(isHostedProjectStatusResponse(envelope({ ...project, job_stage })), true);
  }
  for (const job_state of ["queued", "running", "partial", "complete", "unavailable"] as const) {
    assert.equal(isHostedProjectStatusResponse(envelope({ ...project, job_state })), true);
  }
  for (const evaluation_status of [null, "validated_unpublished", "partial", "unavailable"] as const) {
    assert.equal(isHostedProjectStatusResponse(envelope({ ...project, evaluation_status })), true);
  }
  for (const score_status of ["pending", "unavailable"] as const) {
    assert.equal(isHostedProjectStatusResponse(envelope({ ...project, score_status })), true);
  }
  assert.equal(isHostedProjectStatusResponse(envelope({ ...project, evaluation_status: "validated" })), false);

  const recent = { ...project, collection_status: project.request_state } as Record<string, unknown>;
  for (const key of ["request_id", "request_state", "job_stage", "job_state", "last_error_code", "next_attempt_at"]) {
    delete recent[key];
  }
  assert.equal(isHostedRecentSourcesResponse(envelope([recent])), true);
  for (const collection_status of ["queued", "collecting", "partial", "complete", "unavailable"] as const) {
    assert.equal(isHostedRecentSourcesResponse(envelope([{ ...recent, collection_status }])), true);
  }
  for (const evaluation_status of [null, "validated_unpublished", "partial", "unavailable"] as const) {
    assert.equal(isHostedRecentSourcesResponse(envelope([{ ...recent, evaluation_status }])), true);
  }
  assert.equal(isHostedRecentSourcesResponse(envelope([{ ...recent, catalog_rank: 1 }])), false);
});
