// TypeScript mirror of schemas/project-ai-analysis/v1.json.
// This is a bounded interpretation of cited public evidence, never a score.

export type ProjectAiApplicability =
  | "applicable"
  | "partially_applicable"
  | "not_applicable";

export interface ProjectAiAnalysisJudgment {
  criterion_id: string;
  applicability: ProjectAiApplicability;
  rating: number | null;
  rating_scale: 4;
  confidence: number;
  evidence_ids: string[];
  rationale: string;
}

export interface ProjectAiAnalysis {
  project: { owner: string; repository: string };
  revision: { commit_sha: string; default_branch: string };
  evaluation: {
    evaluation_version: string;
    rubric_version: string;
    evidence_bundle_hash: string;
  };
  interpretation: { kind: "ai_interpretation"; not_a_project_score: true };
  judgments: ProjectAiAnalysisJudgment[];
  limitations: string[];
}

export interface ProjectAiAnalysisEnvelope {
  contract: "project-ai-analysis";
  schema_version: "1.0.0";
  data: ProjectAiAnalysis;
}
