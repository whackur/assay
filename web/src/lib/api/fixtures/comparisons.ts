// ProjectComparison fixtures: functional-cohort comparisons for the featured
// Hermes card and the canonical scored sample, with detailed and additional
// candidates exercising every facet status (complete and unavailable).

import type {
  CanonicalFacet,
  DetailedCandidate,
  HostedSource,
  ProjectComparison,
} from "@/lib/contract/types";
import { REVISION, SNAPSHOT, source } from "./builders";
import { scoredRepo } from "./core-evaluations";
import { hermesRepo } from "./featured-evaluations";

const CANDIDATE_REVISION = "fedcba9876543210fedcba9876543210fedcba98";

function candidateEvidence(namespace: string, repository: string): string {
  return `evidence:github:candidate-${namespace}-${repository}`;
}

function detailedCandidate(
  namespace: string,
  repository: string,
  overall: number,
  confidence: number,
  stars: number | null,
  facets: [CanonicalFacet, number | null][],
): DetailedCandidate {
  const cev = candidateEvidence(namespace, repository);
  return {
    candidate: {
      source: source(namespace, repository),
      revision: CANDIDATE_REVISION,
    },
    selection_reasons: facets.filter(([, v]) => v !== null).map(([f]) => f),
    confidence,
    overall_similarity: { status: "complete", value: overall },
    facets: facets.map(([facet, value]) => ({
      facet,
      status: value === null ? "unavailable" : "complete",
      value,
    })),
    popularity: { stars },
    differentiators: {
      seed_only: [{ token: "json_output", evidence_ids: [SNAPSHOT] }],
      candidate_only: [{ token: "web_dashboard", evidence_ids: [cev] }],
    },
    evidence_ids: [cev, SNAPSHOT],
  };
}

const [PROBLEM_OVERLAP, FEATURE_OVERLAP, TECHNICAL_SIMILARITY, STRUCTURAL_SIMILARITY] = [
  "problem_overlap",
  "feature_overlap",
  "technical_similarity",
  "structural_similarity",
] as const satisfies CanonicalFacet[];

function functionalCohort(seed: HostedSource, candidates: DetailedCandidate[]): ProjectComparison {
  return {
    schema_version: "1.0.0",
    comparison_version: "project-comparison-1",
    mode: "functional_cohort",
    status: "partial",
    search_depth: "one_depth",
    seed: { source: seed, revision: REVISION },
    facet_weights: [
      { facet: "problem_overlap", weight: 30 },
      { facet: "feature_overlap", weight: 30 },
      { facet: "technical_similarity", weight: 20 },
      { facet: "structural_similarity", weight: 20 },
    ],
    detailed_candidates: candidates,
    additional_candidates: [
      {
        candidate: { source: source("other-org", "adjacent-tool"), revision: CANDIDATE_REVISION },
        confidence: 0.4,
        overall_similarity: { status: "complete", value: 0.3 },
        evidence_ids: [candidateEvidence("other-org", "adjacent-tool")],
      },
    ],
    evidence_ids: [...candidates.flatMap((c) => c.evidence_ids), SNAPSHOT],
    warnings: [],
    limitations: [
      { code: "candidate_similarity_insufficient", evidence_ids: [candidateEvidence("other-org", "unrelated")] },
    ],
  };
}

export const COMPARISONS: Record<string, ProjectComparison> = {
  "hermes-agent/hermes": functionalCohort(hermesRepo, [
    detailedCandidate("other-org", "autopilot", 0.78, 0.75, 4200, [
      [PROBLEM_OVERLAP, 0.9],
      [FEATURE_OVERLAP, 0.7],
      [TECHNICAL_SIMILARITY, 0.8],
      [STRUCTURAL_SIMILARITY, null],
    ]),
    detailedCandidate("other-org", "chatops-bot", 0.62, 0.6, 1200, [
      [PROBLEM_OVERLAP, 0.6],
      [FEATURE_OVERLAP, 0.5],
      [TECHNICAL_SIMILARITY, null],
      [STRUCTURAL_SIMILARITY, 0.7],
    ]),
    detailedCandidate("other-org", "task-runner", 0.5, 0.4, 300, [
      [PROBLEM_OVERLAP, 0.5],
      [FEATURE_OVERLAP, null],
      [TECHNICAL_SIMILARITY, 0.5],
      [STRUCTURAL_SIMILARITY, null],
    ]),
  ]),
  "example-org/sample-project": functionalCohort(scoredRepo, [
    detailedCandidate("other-org", "repo-scan", 0.7, 0.7, 800, [
      [PROBLEM_OVERLAP, 0.8],
      [FEATURE_OVERLAP, 0.6],
      [TECHNICAL_SIMILARITY, null],
      [STRUCTURAL_SIMILARITY, 0.7],
    ]),
    detailedCandidate("other-org", "code-audit", 0.55, 0.5, 150, [
      [PROBLEM_OVERLAP, 0.5],
      [FEATURE_OVERLAP, 0.6],
      [TECHNICAL_SIMILARITY, 0.5],
      [STRUCTURAL_SIMILARITY, null],
    ]),
  ]),
};