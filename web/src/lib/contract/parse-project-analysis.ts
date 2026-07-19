// Parser for schemas/project-analysis/v1.json. Validates the bundle shape and
// delegates to the manifest parser for the nested manifest instance. Evidence
// instances are validated inline to avoid a circular import with ./parse.

import type { ProjectAnalysis } from "@/lib/contract/types";
import {
  asRecord,
  isRecord,
  require,
  requireArray,
  requireSchemaVersion,
} from "./parse-helpers";
import { parseManifest } from "./parse-analysis-manifest";

export function parseAnalysis(input: unknown): ProjectAnalysis {
  const value = asRecord(input, "project-analysis");
  requireSchemaVersion(value, "project-analysis");
  require(isRecord(value.manifest), "project-analysis manifest is required");
  parseManifest(value.manifest);
  const evidence = requireArray(value, "evidence", "project-analysis");
  require(evidence.length >= 1, "project-analysis has at least one evidence instance");
  for (const e of evidence) requireEvidence(e);
  if (value.evaluation !== undefined) {
    require(isRecord(value.evaluation), "project-analysis evaluation must be an object");
  }
  return input as unknown as ProjectAnalysis;
}

function requireEvidence(input: unknown): void {
  const e = asRecord(input, "evidence");
  require(
    typeof e.schema_version === "string" && /^1\.\d+\.\d+$/.test(e.schema_version),
    "unsupported project-evidence schema_version",
  );
  require(
    typeof e.id === "string" && e.id.startsWith("evidence:"),
    "evidence id must be an evidence reference",
  );
  require(typeof e.status === "string", "evidence status is required");
}