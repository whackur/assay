// TypeScript mirror of schemas/analysis-manifest/v1.json. The manifest
// describes repository evidence and analysis sufficiency; it never measures a
// person's value, effort, productivity, or performance.

import type { RepositorySource, Status } from "./common";

export type AnalysisScopeMode = "single_revision" | "revision_range";

export type RequestedCapability =
  | "repository_snapshot"
  | "tracked_files"
  | "file_classification"
  | "repository_history"
  | "language_detection"
  | "reported_ci";

export type DataSourceKind =
  | "repository"
  | "repository_content"
  | "repository_history"
  | "platform_record"
  | "reported_ci"
  | "release_artifact"
  | "documentation";

export type DataSourceVisibility = "public" | "private_local";

export type DataSourceRetention = "metadata_only" | "content_addressed_cache";

export type ArtifactRole = "project_evidence" | "analysis_diagnostics";

export interface ManifestTool {
  name: "assay";
  version: string;
}

export interface ManifestComponent {
  name: string;
  version: string;
}

export interface ManifestRevision {
  revision: string;
  root_tree: string | null;
  commit_time: string;
  source: RepositorySource;
}

export interface ManifestScope {
  mode: AnalysisScopeMode;
  base_revision: string | null;
  head_revision: string;
  history_status: Status;
  commit_count: number | null;
  requested_capabilities: RequestedCapability[];
}

export interface ManifestDataSource {
  id: string;
  kind: DataSourceKind;
  status: Status;
  revision: string | null;
  content_hash: string | null;
  remote_record_id: string | null;
  collected_at: string;
  visibility: DataSourceVisibility;
  retention: DataSourceRetention;
}

export interface ManifestArtifact {
  role: ArtifactRole;
  schema_version: string;
  content_hash: string;
  record_count: number;
  status: Status;
}

export interface ManifestDiagnostic {
  code: string;
  affected_evidence_ids: string[];
}

export interface AnalysisManifest {
  schema_version: string;
  analysis_version: string;
  tool: ManifestTool;
  source_snapshot: ManifestRevision;
  rule_set_hash: string;
  config_hash: string;
  analyzers: ManifestComponent[];
  parsers: ManifestComponent[];
  status: Status;
  generated_at: string;
  scope: ManifestScope;
  data_sources: ManifestDataSource[];
  artifacts: ManifestArtifact[];
  warnings: ManifestDiagnostic[];
  limitations: ManifestDiagnostic[];
}