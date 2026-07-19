import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseManifest } from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real analysis-manifest golden fixture", () => {
  const manifest = parseManifest(golden("analysis-manifest-v1.json"));
  assert.equal(manifest.status, "partial");
  assert.equal(manifest.tool.name, "assay");
  assert.equal(manifest.scope.mode, "single_revision");
  assert.equal(manifest.data_sources.length, 2);
  assert.equal(manifest.artifacts[0]?.role, "project_evidence");
  assert.equal(manifest.warnings[0]?.code, "history_depth_limited");
});

test("rejects a manifest with a non-assay tool name", () => {
  const fixture = golden("analysis-manifest-v1.json") as Record<string, unknown>;
  const tool = { ...(fixture.tool as object), name: "other" };
  assert.throws(
    () => parseManifest({ ...fixture, tool }),
    ContractError,
  );
});

test("rejects a manifest with a malformed rule_set_hash", () => {
  const fixture = golden("analysis-manifest-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseManifest({ ...fixture, rule_set_hash: "sha256:bad" }),
    ContractError,
  );
});

test("rejects a complete manifest with no artifacts", () => {
  const fixture = golden("analysis-manifest-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseManifest({ ...fixture, status: "complete", artifacts: [] }),
    ContractError,
  );
});

test("rejects a manifest with an invalid data source kind", () => {
  const fixture = golden("analysis-manifest-v1.json") as Record<string, unknown>;
  const dataSources = [...(fixture.data_sources as object[])];
  dataSources[0] = { ...dataSources[0], kind: "unknown" };
  assert.throws(
    () => parseManifest({ ...fixture, data_sources: dataSources }),
    ContractError,
  );
});

test("rejects a manifest with an unsupported schema version", () => {
  const fixture = golden("analysis-manifest-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseManifest({ ...fixture, schema_version: "2.0.0" }),
    ContractError,
  );
});