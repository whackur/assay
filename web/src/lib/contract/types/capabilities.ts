// TypeScript mirror of schemas/capabilities/v1.json. Declares the commands,
// formats, schemas, languages, and feature flags the build advertises.

export type CapabilitiesCommand =
  | "project analyze"
  | "capabilities"
  | "serve"
  | "history";

export type CapabilitiesSchemaName =
  | "analysis-manifest"
  | "capabilities"
  | "project-analysis"
  | "project-evidence"
  | "project-evaluation";

export type FeatureId =
  | "ai_evaluation"
  | "attribute_resolution"
  | "file_classification"
  | "github_collection"
  | "local_git_snapshot"
  | "local_private_history"
  | "loopback_dashboard"
  | "project_scores"
  | "remote_private_fetch"
  | "repository_code_execution"
  | "semantic_diff";

export type FeatureStatus = "implemented" | "not_implemented" | "prohibited";

export type EvaluatorFamily = "deterministic" | "api_key" | "agentic";

export type EvaluatorStatus = "implemented" | "not_implemented";

export interface CapabilitiesTool {
  name: "assay";
  version: string;
}

export interface CapabilitiesSchema {
  name: CapabilitiesSchemaName;
  version: "1.0.0";
}

export interface CapabilitiesEvaluator {
  id: string;
  family: EvaluatorFamily;
  status: EvaluatorStatus;
}

export interface Feature {
  id: FeatureId;
  status: FeatureStatus;
  evaluators?: CapabilitiesEvaluator[];
}

export interface Capabilities {
  schema_version: string;
  tool: CapabilitiesTool;
  commands: CapabilitiesCommand[];
  formats: ["json"];
  schemas: CapabilitiesSchema[];
  languages: ["javascript", "python", "tsx", "typescript"];
  features: Feature[];
}