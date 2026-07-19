// TypeScript mirror of schemas/project-analysis/v1.json. One CLI result whose
// bundled same-origin references require an analysis-manifest instance and
// project-evidence instances without network resolution. The optional
// evaluation field carries a project-evaluation instance when the wired
// evaluator and score compiler ran.

import type { AnalysisManifest } from "./analysis-manifest";
import type { ProjectEvaluation } from "./project-evaluation";
import type { ProjectEvidence } from "./project-evidence";

export interface ProjectAnalysis {
  schema_version: string;
  manifest: AnalysisManifest;
  evidence: ProjectEvidence[];
  evaluation?: ProjectEvaluation;
}