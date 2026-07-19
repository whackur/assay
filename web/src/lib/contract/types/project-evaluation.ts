// TypeScript mirror of schemas/project-evaluation/v1.json. The web app renders
// these; it never recomputes any score or metric.

import type {
  Diagnostic,
  EvaluatorProvider,
  ProjectRef,
  Status,
  Visibility,
} from "./common";

export type PrimaryType =
  | "application"
  | "library_sdk_framework"
  | "cli_developer_tool"
  | "service_infrastructure_platform"
  | "curated_resource"
  | "protocol_specification_standard"
  | "dataset_model_research_artifact"
  | "educational_example_template"
  | "experimental_proof_of_concept";

export type Maturity =
  | "concept"
  | "prototype"
  | "alpha"
  | "beta"
  | "stable"
  | "maintenance"
  | "dormant"
  | "archived";

export interface Evaluator {
  profile: string;
  provider: EvaluatorProvider;
  model: string | null;
  rubric_version: string;
}

export interface Compiler {
  version: string;
  rule_set_hash: string;
  judgment_bundle_hash: string | null;
}

export interface Classification {
  status: Status;
  primary_type: PrimaryType | null;
  secondary_types: PrimaryType[];
  tags: string[];
  maturity: Maturity | null;
  confidence: number;
  evidence_ids: string[];
}

export interface Score {
  status: Status;
  value: number | null;
  confidence: number;
  version: string;
  evidence_ids: string[];
}

export interface CitedStatement {
  text: string;
  evidence_ids: string[];
}

export interface Potential extends Score {
  forecast_horizon: string;
  assumptions: CitedStatement[];
  major_counter_signals: CitedStatement[];
}

export interface Scores {
  assay_score: Score;
  project_substance: Score;
  originality: Score;
  engineering_rigor: Score;
  open_source_readiness: Score;
  maintenance_health: Score;
  potential: Potential;
}

export interface Introduction {
  status: Status;
  factual_statements: CitedStatement[];
  interpretations: CitedStatement[];
}

export interface ProjectEvaluation {
  schema_version: string;
  evaluation_version: string;
  status: Status;
  provisional: boolean;
  visibility: Visibility;
  evaluator: Evaluator;
  compiler: Compiler;
  project: ProjectRef;
  classification: Classification;
  scores: Scores;
  evidence_ids: string[];
  introduction: Introduction;
  warnings: Diagnostic[];
  limitations: Diagnostic[];
}