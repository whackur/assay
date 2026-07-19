// Parser for schemas/analysis-manifest/v1.json. Validates load-bearing fields
// and the conditional rules the web app renders (e.g. complete status requires
// complete data sources and at least one complete artifact).

import type { AnalysisManifest } from "@/lib/contract/types";
import {
  asRecord,
  isSha256,
  isTimestamp,
  isVersionIdentifier,
  require,
  requireArray,
  requireRecord,
  requireString,
  requireSchemaVersion,
} from "./parse-helpers";

const STATUS_VALUES = new Set([
  "complete",
  "partial",
  "unavailable",
  "unsupported",
  "insufficient",
  "pending",
]);

const DATA_SOURCE_KINDS = new Set([
  "repository",
  "repository_content",
  "repository_history",
  "platform_record",
  "reported_ci",
  "release_artifact",
  "documentation",
]);

const ARTIFACT_ROLES = new Set(["project_evidence", "analysis_diagnostics"]);

export function parseManifest(input: unknown): AnalysisManifest {
  const value = asRecord(input, "analysis-manifest");
  requireSchemaVersion(value, "analysis-manifest");
  requireString(value, "analysis_version", "analysis-manifest");
  require(isVersionIdentifier(value.analysis_version), "analysis_version is malformed");
  requireRecord(value, "tool", "analysis-manifest");
  const tool = value.tool as Record<string, unknown>;
  require(tool.name === "assay", "manifest tool.name must be assay");
  require(typeof tool.version === "string", "manifest tool.version is required");
  requireRecord(value, "source_snapshot", "analysis-manifest");
  require(isSha256(value.rule_set_hash), "rule_set_hash must be a sha256");
  require(isSha256(value.config_hash), "config_hash must be a sha256");

  const analyzers = requireArray(value, "analyzers", "analysis-manifest");
  require(analyzers.length >= 1, "manifest has at least one analyzer");
  for (const a of analyzers) requireComponent(a, "analyzer");
  const parsers = requireArray(value, "parsers", "analysis-manifest");
  for (const p of parsers) requireComponent(p, "parser");

  require(typeof value.status === "string" && STATUS_VALUES.has(value.status), "manifest status is invalid");
  require(typeof value.generated_at === "string" && isTimestamp(value.generated_at), "generated_at must be a timestamp");
  requireRecord(value, "scope", "analysis-manifest");
  requireScope(value.scope);

  const dataSources = requireArray(value, "data_sources", "analysis-manifest");
  require(dataSources.length >= 1, "manifest has at least one data source");
  for (const ds of dataSources) requireDataSource(ds);

  const artifacts = requireArray(value, "artifacts", "analysis-manifest");
  for (const a of artifacts) requireArtifact(a);

  requireArray(value, "warnings", "analysis-manifest");
  requireArray(value, "limitations", "analysis-manifest");

  if (value.status === "complete") {
    require(artifacts.length >= 1, "complete manifest has at least one artifact");
    for (const a of artifacts) {
      const artifact = a as Record<string, unknown>;
      require(artifact.status === "complete", "complete manifest artifacts must be complete");
    }
  }
  return input as unknown as AnalysisManifest;
}

function requireComponent(input: unknown, label: string): void {
  const c = asRecord(input, label);
  require(typeof c.name === "string" && isVersionIdentifier(c.name), `${label} name is invalid`);
  require(typeof c.version === "string" && isVersionIdentifier(c.version), `${label} version is invalid`);
}

function requireScope(input: unknown): void {
  const s = asRecord(input, "scope");
  require(s.mode === "single_revision" || s.mode === "revision_range", "scope mode is invalid");
  require(s.base_revision === null || typeof s.base_revision === "string", "scope base_revision is invalid");
  require(typeof s.head_revision === "string", "scope head_revision is required");
  require(typeof s.history_status === "string" && STATUS_VALUES.has(s.history_status), "scope history_status is invalid");
  require(s.commit_count === null || (typeof s.commit_count === "number" && s.commit_count >= 0), "scope commit_count is invalid");
  const caps = requireArray(s, "requested_capabilities", "scope");
  for (const cap of caps) require(typeof cap === "string", "scope capability must be a string");
}

function requireDataSource(input: unknown): void {
  const ds = asRecord(input, "data source");
  require(typeof ds.id === "string" && /^evidence:[a-z0-9._-]+:[a-z0-9._-]+(?::[a-z0-9._-]+)*$/.test(ds.id), "data source id is invalid");
  require(typeof ds.kind === "string" && DATA_SOURCE_KINDS.has(ds.kind), "data source kind is invalid");
  require(typeof ds.status === "string" && STATUS_VALUES.has(ds.status), "data source status is invalid");
  require(ds.revision === null || typeof ds.revision === "string", "data source revision is invalid");
  require(ds.content_hash === null || isSha256(ds.content_hash), "data source content_hash is invalid");
  require(ds.remote_record_id === null || typeof ds.remote_record_id === "string", "data source remote_record_id is invalid");
  require(typeof ds.collected_at === "string" && isTimestamp(ds.collected_at), "data source collected_at is invalid");
  require(ds.visibility === "public" || ds.visibility === "private_local", "data source visibility is invalid");
  require(ds.retention === "metadata_only" || ds.retention === "content_addressed_cache", "data source retention is invalid");
}

function requireArtifact(input: unknown): void {
  const a = asRecord(input, "artifact");
  require(typeof a.role === "string" && ARTIFACT_ROLES.has(a.role), "artifact role is invalid");
  require(typeof a.schema_version === "string" && /^1\.\d+\.\d+$/.test(a.schema_version), "artifact schema_version is invalid");
  require(isSha256(a.content_hash), "artifact content_hash must be a sha256");
  require(typeof a.record_count === "number" && a.record_count >= 0, "artifact record_count is invalid");
  require(typeof a.status === "string" && STATUS_VALUES.has(a.status), "artifact status is invalid");
}