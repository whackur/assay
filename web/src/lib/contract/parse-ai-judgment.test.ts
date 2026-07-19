import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseAiJudgment } from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real ai-judgment golden fixture", () => {
  const judgment = parseAiJudgment(golden("ai-judgment-v1.json"));
  assert.equal(judgment.status, "complete");
  assert.equal(judgment.rubric_version, "project-rubric-1");
  assert.equal(judgment.privacy.evidence_scope, "public_only");
  assert.equal(judgment.judgments.length, 1);
  assert.equal(judgment.judgments[0]?.rating, 3);
  assert.equal(judgment.judgments[0]?.rating_scale, 4);
  assert.equal(judgment.judgments[0]?.evidence_ids.length, 2);
});

test("rejects an ai-judgment with a malformed evidence_bundle_hash", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseAiJudgment({ ...fixture, evidence_bundle_hash: "sha256:bad" }),
    ContractError,
  );
});

test("rejects an ai-judgment with private_local scope and public_only transmission", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  const privacy = { evidence_scope: "private_local", external_transmission: "public_only" };
  assert.throws(
    () => parseAiJudgment({ ...fixture, privacy }),
    ContractError,
  );
});

test("rejects a not_applicable judgment with a non-null rating", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  const judgments = [...(fixture.judgments as object[])];
  judgments[0] = { ...judgments[0], applicability: "not_applicable", rating: 2 };
  assert.throws(
    () => parseAiJudgment({ ...fixture, judgments }),
    ContractError,
  );
});

test("rejects an applicable judgment with no evidence ids", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  const judgments = [...(fixture.judgments as object[])];
  judgments[0] = { ...judgments[0], evidence_ids: [] };
  assert.throws(
    () => parseAiJudgment({ ...fixture, judgments }),
    ContractError,
  );
});

test("rejects a complete ai-judgment with no judgments", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseAiJudgment({ ...fixture, judgments: [] }),
    ContractError,
  );
});

test("rejects an ai-judgment with an unsupported schema version", () => {
  const fixture = golden("ai-judgment-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseAiJudgment({ ...fixture, schema_version: "2.0.0" }),
    ContractError,
  );
});