// Parser for schemas/capabilities/v1.json. Validates the fixed arrays the
// contract pins (formats, languages) and the enumerated command/schema/feature
// identifiers the web app renders.

import type { Capabilities } from "@/lib/contract/types";
import {
  asRecord,
  require,
  requireArray,
  requireRecord,
  requireSchemaVersion,
} from "./parse-helpers";

const COMMANDS = new Set(["project analyze", "capabilities", "serve", "history"]);
const SCHEMA_NAMES = new Set([
  "analysis-manifest",
  "capabilities",
  "project-analysis",
  "project-evidence",
  "project-evaluation",
]);
const FEATURE_IDS = new Set([
  "ai_evaluation",
  "attribute_resolution",
  "file_classification",
  "github_collection",
  "local_git_snapshot",
  "local_private_history",
  "loopback_dashboard",
  "project_scores",
  "remote_private_fetch",
  "repository_code_execution",
  "semantic_diff",
]);
const FEATURE_STATUS = new Set(["implemented", "not_implemented", "prohibited"]);
const EVALUATOR_FAMILY = new Set(["deterministic", "api_key", "agentic"]);
const EVALUATOR_STATUS = new Set(["implemented", "not_implemented"]);

export function parseCapabilities(input: unknown): Capabilities {
  const value = asRecord(input, "capabilities");
  requireSchemaVersion(value, "capabilities");
  requireRecord(value, "tool", "capabilities");
  const tool = value.tool as Record<string, unknown>;
  require(tool.name === "assay", "capabilities tool.name must be assay");
  require(typeof tool.version === "string", "capabilities tool.version is required");

  const commands = requireArray(value, "commands", "capabilities");
  for (const c of commands) {
    require(typeof c === "string" && COMMANDS.has(c), "capabilities command is invalid");
  }

  const formats = requireArray(value, "formats", "capabilities");
  require(
    formats.length === 1 && formats[0] === "json",
    "capabilities formats must be [\"json\"]",
  );

  const schemas = requireArray(value, "schemas", "capabilities");
  for (const s of schemas) {
    const schema = asRecord(s, "capabilities schema");
    require(typeof schema.name === "string" && SCHEMA_NAMES.has(schema.name), "capabilities schema name is invalid");
    require(schema.version === "1.0.0", "capabilities schema version must be 1.0.0");
  }

  const languages = requireArray(value, "languages", "capabilities");
  require(
    languages.length === 4 &&
      languages[0] === "javascript" &&
      languages[1] === "python" &&
      languages[2] === "tsx" &&
      languages[3] === "typescript",
    "capabilities languages must be [\"javascript\",\"python\",\"tsx\",\"typescript\"]",
  );

  const features = requireArray(value, "features", "capabilities");
  for (const f of features) requireFeature(f);
  return input as unknown as Capabilities;
}

function requireFeature(input: unknown): void {
  const f = asRecord(input, "feature");
  require(typeof f.id === "string" && FEATURE_IDS.has(f.id), "feature id is invalid");
  require(typeof f.status === "string" && FEATURE_STATUS.has(f.status), "feature status is invalid");
  if (f.evaluators !== undefined) {
    require(Array.isArray(f.evaluators), "feature evaluators must be an array");
    for (const e of f.evaluators as unknown[]) {
      const ev = asRecord(e, "evaluator");
      require(typeof ev.id === "string" && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(ev.id), "evaluator id is invalid");
      require(typeof ev.family === "string" && EVALUATOR_FAMILY.has(ev.family), "evaluator family is invalid");
      require(typeof ev.status === "string" && EVALUATOR_STATUS.has(ev.status), "evaluator status is invalid");
    }
  }
}