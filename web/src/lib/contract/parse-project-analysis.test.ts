import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseAnalysis } from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real project-analysis golden fixture", () => {
  const analysis = parseAnalysis(golden("project-analysis-v1.json"));
  assert.equal(analysis.manifest.status, "complete");
  assert.equal(analysis.evidence.length, 2);
  assert.equal(analysis.evidence[0]?.id, "evidence:history-scope:v1-golden");
  assert.equal(analysis.evaluation, undefined);
});

test("rejects a project-analysis with no evidence", () => {
  const fixture = golden("project-analysis-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseAnalysis({ ...fixture, evidence: [] }),
    ContractError,
  );
});

test("rejects a project-analysis with a malformed nested manifest", () => {
  const fixture = golden("project-analysis-v1.json") as Record<string, unknown>;
  const manifest = { ...(fixture.manifest as object), tool: { name: "other", version: "0.1.0" } };
  assert.throws(
    () => parseAnalysis({ ...fixture, manifest }),
    ContractError,
  );
});

test("rejects a project-analysis with an unsupported schema version", () => {
  const fixture = golden("project-analysis-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseAnalysis({ ...fixture, schema_version: "2.0.0" }),
    ContractError,
  );
});