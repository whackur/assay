// TypeScript mirrors of the versioned Assay JSON contracts under schemas/.
// Field names stay snake_case to match the machine-readable schema exactly.
// The web app renders these; it never recomputes any score or metric.

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

export interface Diagnostic {
  code: string;
  evidence_ids: string[];
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

export type ComparisonMode = "functional_cohort" | "curated_list";

export type CanonicalFacet =
  | "problem_overlap"
  | "feature_overlap"
  | "technical_similarity"
  | "structural_similarity"
  | "entry_overlap"
  | "list_structure"
  | "unique_coverage"
  | "editorial_quality"
  | "maintenance_evidence";

export interface HostedProjectRef {
  source: HostedSource;
  revision: string;
}

export interface FacetWeight {
  facet: CanonicalFacet;
  weight: number;
}

export interface Similarity {
  status: Status;
  value: number | null;
}

export interface FacetSimilarity extends Similarity {
  facet: CanonicalFacet;
}

export interface Differentiator {
  token: string;
  evidence_ids: string[];
}

export interface Popularity {
  stars: number | null;
}

export interface DetailedCandidate {
  candidate: HostedProjectRef;
  selection_reasons: CanonicalFacet[];
  confidence: number;
  overall_similarity: Similarity;
  facets: FacetSimilarity[];
  popularity: Popularity;
  differentiators: {
    seed_only: Differentiator[];
    candidate_only: Differentiator[];
  };
  evidence_ids: string[];
}

export interface CompactCandidate {
  candidate: HostedProjectRef;
  confidence: number;
  overall_similarity: Similarity;
  evidence_ids: string[];
}

export interface ProjectComparison {
  schema_version: string;
  comparison_version: string;
  mode: ComparisonMode;
  status: Status;
  search_depth: "one_depth";
  seed: ProjectRef;
  facet_weights: FacetWeight[];
  detailed_candidates: DetailedCandidate[];
  additional_candidates: CompactCandidate[];
  evidence_ids: string[];
  warnings: Diagnostic[];
  limitations: Diagnostic[];
}

export type EvidenceGrade = "a" | "b" | "c" | "d" | null;

export interface EvidencePrivacy {
  visibility: "public" | "private_local";
  source_content: "not_retained" | "content_addressed_cache" | "explicit_retention";
  external_transmission:
    | "not_requested"
    | "prohibited"
    | "consent_required"
    | "consented";
}

export interface EvidenceProvenance {
  source_kind: string;
  collected_at: string;
  repository_revision: string | null;
  content_hash: string | null;
  remote_record_id: string | null;
}

export interface ProjectEvidence {
  schema_version: string;
  repository: RepositorySource;
  id: string;
  status: Status;
  grade: EvidenceGrade;
  privacy: EvidencePrivacy;
  provenance?: EvidenceProvenance;
  payload?: { kind: string; [key: string]: unknown };
  requested_kind?: string;
  reason?: string;
}
