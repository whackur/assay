// TypeScript mirror of schemas/project-comparison/v1.json.

import type {
  Diagnostic,
  HostedProjectRef,
  ProjectRef,
  Status,
} from "./common";

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