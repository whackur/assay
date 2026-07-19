import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { ContractError, parseCapabilities } from "@/lib/contract/parse";

const goldenDir = join(dirname(fileURLToPath(import.meta.url)), "../../../../tests/golden");

function golden(name: string): unknown {
  return JSON.parse(readFileSync(join(goldenDir, name), "utf8"));
}

test("parses the real capabilities golden fixture", () => {
  const capabilities = parseCapabilities(golden("capabilities-v1.json"));
  assert.equal(capabilities.tool.name, "assay");
  assert.deepEqual(capabilities.formats, ["json"]);
  assert.equal(capabilities.schemas.length, 5);
  assert.equal(capabilities.features[0]?.id, "ai_evaluation");
  assert.equal(capabilities.features[0]?.evaluators?.length, 3);
  assert.equal(capabilities.features[9]?.status, "prohibited");
});

test("rejects capabilities with a non-assay tool name", () => {
  const fixture = golden("capabilities-v1.json") as Record<string, unknown>;
  const tool = { ...(fixture.tool as object), name: "other" };
  assert.throws(
    () => parseCapabilities({ ...fixture, tool }),
    ContractError,
  );
});

test("rejects capabilities with an unexpected format", () => {
  const fixture = golden("capabilities-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseCapabilities({ ...fixture, formats: ["yaml"] }),
    ContractError,
  );
});

test("rejects capabilities with an unknown command", () => {
  const fixture = golden("capabilities-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseCapabilities({ ...fixture, commands: ["unknown"] }),
    ContractError,
  );
});

test("rejects capabilities with an unknown feature id", () => {
  const fixture = golden("capabilities-v1.json") as Record<string, unknown>;
  const features = [...(fixture.features as object[])];
  features[0] = { ...features[0], id: "unknown_feature" };
  assert.throws(
    () => parseCapabilities({ ...fixture, features }),
    ContractError,
  );
});

test("rejects capabilities with an unsupported schema version", () => {
  const fixture = golden("capabilities-v1.json") as Record<string, unknown>;
  assert.throws(
    () => parseCapabilities({ ...fixture, schema_version: "2.0.0" }),
    ContractError,
  );
});