import type {
  HostedSource,
  ProjectEvaluation,
  ProjectEvidence,
  Score,
  Status,
} from "@/lib/contract/types";
import type { EvaluationRecord } from "@/lib/api/client";
import type { EvaluatorProfileKind } from "@/lib/state/cooldown";

// Contract-based development and demo fixtures. These conform to
// schemas/project-evaluation/v1.json and schemas/project-evidence/v1.json.
// They carry no metric logic; scores are fixed authored values.

const REVISION = "0123456789abcdef0123456789abcdef01234567";
const SNAPSHOT = "evidence:repository:snapshot";

function score(value: number | null, confidence: number, status: Status = "complete"): Score {
  return {
    status,
    value,
    confidence,
    version: "project-score-1",
    evidence_ids: value === null ? [] : [SNAPSHOT],
  };
}

function source(namespace: string, repository: string): HostedSource {
  return { kind: "hosted", provider: "github", namespace, repository };
}

function snapshotEvidence(repo: HostedSource): ProjectEvidence {
  return {
    schema_version: "1.0.0",
    repository: repo,
    id: SNAPSHOT,
    status: "complete",
    grade: "a",
    privacy: {
      visibility: "public",
      source_content: "not_retained",
      external_transmission: "not_requested",
    },
    provenance: {
      source_kind: "repository",
      collected_at: "2026-07-14T00:00:00Z",
      repository_revision: REVISION,
      content_hash: null,
      remote_record_id: null,
    },
    payload: { kind: "repository_snapshot", commit_time: "2026-07-13T12:00:00Z", root_tree: REVISION },
  };
}

function fileEvidence(repo: HostedSource, path: string, grade: "a" | "b" | "c"): ProjectEvidence {
  return {
    schema_version: "1.0.0",
    repository: repo,
    id: `evidence:file:${path.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}`,
    status: "complete",
    grade,
    privacy: {
      visibility: "public",
      source_content: "not_retained",
      external_transmission: "not_requested",
    },
    provenance: {
      source_kind: "repository_content",
      collected_at: "2026-07-14T00:00:00Z",
      repository_revision: REVISION,
      content_hash: "sha256:" + "4".repeat(64),
      remote_record_id: null,
    },
    payload: {
      kind: "file",
      relative_path: path,
      language: "TypeScript",
      language_status: "complete",
      size_bytes: 418,
      content_hash: "sha256:" + "4".repeat(64),
      classification: {
        primary_category: "production_code",
        tags: ["production"],
        rule_id: "path.production.typescript",
        confidence: 1.0,
      },
    },
  };
}

function featureEvidence(repo: HostedSource, feature: string, present: boolean): ProjectEvidence {
  return {
    schema_version: "1.0.0",
    repository: repo,
    id: `evidence:feature:${feature}`,
    status: "complete",
    grade: "b",
    privacy: {
      visibility: "public",
      source_content: "not_retained",
      external_transmission: "not_requested",
    },
    provenance: {
      source_kind: "repository",
      collected_at: "2026-07-14T00:00:00Z",
      repository_revision: REVISION,
      content_hash: null,
      remote_record_id: null,
    },
    payload: {
      kind: "repository_feature",
      feature,
      state: present ? "present" : "absent",
      related_evidence_ids: present ? [SNAPSHOT] : [],
    },
  };
}

const scoredRepo = source("example-org", "sample-project");
const scoredEvaluation: ProjectEvaluation = {
  schema_version: "1.0.0",
  evaluation_version: "project-intelligence-1",
  status: "complete",
  provisional: false,
  visibility: "public",
  evaluator: {
    profile: "openai-project-evaluator-1",
    provider: "openai_api",
    model: "gpt-project-eval",
    rubric_version: "project-rubric-1",
  },
  compiler: {
    version: "project-score-compiler-1",
    rule_set_hash: "sha256:" + "a".repeat(64),
    judgment_bundle_hash: "sha256:" + "b".repeat(64),
  },
  project: { source: scoredRepo, revision: REVISION },
  classification: {
    status: "complete",
    primary_type: "cli_developer_tool",
    secondary_types: ["library_sdk_framework"],
    tags: ["developer_tool"],
    maturity: "beta",
    confidence: 0.82,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(74, 0.72),
    project_substance: score(78, 0.8),
    originality: score(61, 0.55),
    engineering_rigor: score(81, 0.7),
    open_source_readiness: score(69, 0.66),
    maintenance_health: score(64, 0.6),
    potential: {
      ...score(58, 0.5),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [
        { text: "Release cadence continues at the current pace.", evidence_ids: [SNAPSHOT] },
      ],
      major_counter_signals: [
        { text: "A single maintainer authors most changes.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  evidence_ids: [SNAPSHOT],
  introduction: {
    status: "complete",
    factual_statements: [
      { text: "The project ships a command-line interface with JSON output.", evidence_ids: [SNAPSHOT] },
    ],
    interpretations: [
      { text: "Its scope targets developers automating repository analysis.", evidence_ids: [SNAPSHOT] },
    ],
  },
  warnings: [],
  limitations: [
    { code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] },
  ],
};

// The anonymous public golden case: partial, insufficient release gate.
const partialRepo = source("example-org", "early-prototype");
const partialEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  status: "partial",
  visibility: "public",
  evaluator: {
    profile: "deterministic-project-evaluator-1",
    provider: "deterministic",
    model: null,
    rubric_version: "project-rubric-1",
  },
  project: { source: partialRepo, revision: REVISION },
  classification: {
    status: "complete",
    primary_type: "experimental_proof_of_concept",
    secondary_types: [],
    tags: ["prototype"],
    maturity: "prototype",
    confidence: 0.6,
    evidence_ids: [SNAPSHOT],
  },
  scores: {
    assay_score: score(null, 0, "insufficient"),
    project_substance: score(null, 0, "insufficient"),
    originality: score(null, 0, "unavailable"),
    engineering_rigor: score(null, 0, "insufficient"),
    open_source_readiness: score(null, 0, "insufficient"),
    maintenance_health: score(null, 0, "unavailable"),
    potential: {
      ...score(null, 0, "unavailable"),
      version: "potential-1",
      forecast_horizon: "P1Y",
      assumptions: [],
      major_counter_signals: [
        { text: "The current evidence is insufficient for a numeric forecast.", evidence_ids: [SNAPSHOT] },
      ],
    },
  },
  introduction: { status: "unavailable", factual_statements: [], interpretations: [] },
  warnings: [{ code: "score_release_gate_not_met", evidence_ids: [] }],
  limitations: [{ code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] }],
};

// Authenticated private preview, provisional and provider-degraded.
const degradedRepo = source("acme", "degraded");
const degradedEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  status: "partial",
  provisional: true,
  visibility: "private_preview",
  project: { source: degradedRepo, revision: REVISION },
  scores: {
    ...scoredEvaluation.scores,
    originality: score(null, 0, "unavailable"),
    potential: {
      ...scoredEvaluation.scores.potential,
      status: "unavailable",
      value: null,
      evidence_ids: [],
    },
  },
  warnings: [{ code: "provider_unavailable", evidence_ids: [] }],
  limitations: [{ code: "repository_code_not_executed", evidence_ids: [SNAPSHOT] }],
};

// A recently analyzed anonymous public result. A duplicate submission inside
// the 14-day anonymous refresh cooldown cannot force a new run (spec 12.3).
const recentRepo = source("acme", "recently-analyzed");
const recentEvaluation: ProjectEvaluation = {
  ...scoredEvaluation,
  project: { source: recentRepo, revision: REVISION },
};

const inFlightRepo = source("acme", "in-progress");

export const RECORDS: Record<string, EvaluationRecord> = {
  "example-org/sample-project": {
    id: "example-org/sample-project",
    state: "complete",
    evaluation: scoredEvaluation,
    evidence: [
      snapshotEvidence(scoredRepo),
      fileEvidence(scoredRepo, "src/main.ts", "a"),
      fileEvidence(scoredRepo, "src/cli.ts", "b"),
      featureEvidence(scoredRepo, "readme", true),
      featureEvidence(scoredRepo, "license", true),
      featureEvidence(scoredRepo, "ci", true),
      featureEvidence(scoredRepo, "security_policy", false),
    ],
  },
  "example-org/early-prototype": {
    id: "example-org/early-prototype",
    state: "complete",
    evaluation: partialEvaluation,
    evidence: [snapshotEvidence(partialRepo), featureEvidence(partialRepo, "readme", true)],
  },
  "acme/degraded": {
    id: "acme/degraded",
    state: "complete",
    evaluation: degradedEvaluation,
    evidence: [snapshotEvidence(degradedRepo), fileEvidence(degradedRepo, "src/index.ts", "b")],
  },
  "acme/recently-analyzed": {
    id: "acme/recently-analyzed",
    state: "complete",
    evaluation: recentEvaluation,
    evidence: [snapshotEvidence(recentRepo), fileEvidence(recentRepo, "src/app.ts", "a")],
  },
  "acme/in-progress": {
    id: "acme/in-progress",
    state: "in_flight",
    job: {
      stage: "evaluating",
      started_at: "2026-07-16T04:00:00Z",
      canonical: inFlightRepo,
      canonical_url: "https://github.com/acme/in-progress",
      revision: REVISION,
      profile: "anonymous",
    },
  },
};

export function findRecordId(target: HostedSource): string | null {
  const key = `${target.namespace}/${target.repository}`;
  return key in RECORDS ? key : null;
}

export interface CooldownFixture {
  profile: EvaluatorProfileKind;
  last_run_at: string;
}

// Last-run metadata that drives the refresh cooldown state for a matched cache
// hit. Absent means no recent run to gate against.
export const SUBMISSION_COOLDOWNS: Record<string, CooldownFixture> = {
  "acme/recently-analyzed": { profile: "anonymous", last_run_at: "2026-07-14T00:00:00Z" },
};
