// Shared primitives used across every versioned Assay contract: status,
// visibility, repository sources, and project references. Field names stay
// snake_case to match the machine-readable schema exactly.

export type Status =
  | "complete"
  | "partial"
  | "unavailable"
  | "unsupported"
  | "insufficient"
  | "pending";

export type Visibility = "public" | "private_preview" | "private_local";

export type EvaluatorProvider =
  | "deterministic"
  | "openai_api"
  | "codex_cli"
  | "codex_oauth";

export interface HostedSource {
  kind: "hosted";
  provider: string;
  namespace: string;
  repository: string;
}

export interface LocalSource {
  kind: "local";
  repository_id: string;
}

export type RepositorySource = HostedSource | LocalSource;

export interface ProjectRef {
  source: RepositorySource;
  revision: string;
}

export interface HostedProjectRef {
  source: HostedSource;
  revision: string;
}

export interface Diagnostic {
  code: string;
  evidence_ids: string[];
}