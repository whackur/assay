// EvaluationRecord registry and submission-cooldown fixtures. The records
// combine the authored evaluations with their evidence bundles and the
// in-flight job state, keyed by "<namespace>/<repository>".

import type { EvaluationRecord } from "@/lib/api/client";
import type { EvaluatorProfileKind } from "@/lib/state/cooldown";
import type { HostedSource } from "@/lib/contract/types";
import { REVISION, featureEvidence, fileEvidence, snapshotEvidence } from "./builders";
import {
  degradedEvaluation,
  degradedRepo,
  inFlightRepo,
  partialEvaluation,
  partialRepo,
  recentEvaluation,
  recentRepo,
  scoredEvaluation,
  scoredRepo,
} from "./core-evaluations";
import {
  hermesEvaluation,
  hermesRepo,
  openclawEvaluation,
  openclawRepo,
} from "./featured-evaluations";

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
  "hermes-agent/hermes": {
    id: "hermes-agent/hermes",
    state: "complete",
    evaluation: hermesEvaluation,
    evidence: [
      snapshotEvidence(hermesRepo),
      fileEvidence(hermesRepo, "src/agent.ts", "a"),
      fileEvidence(hermesRepo, "src/gateway.ts", "a"),
      featureEvidence(hermesRepo, "readme", true),
      featureEvidence(hermesRepo, "license", true),
      featureEvidence(hermesRepo, "ci", true),
      featureEvidence(hermesRepo, "security_policy", true),
    ],
  },
  "openclaw/openclaw": {
    id: "openclaw/openclaw",
    state: "complete",
    evaluation: openclawEvaluation,
    evidence: [
      snapshotEvidence(openclawRepo),
      fileEvidence(openclawRepo, "src/main.ts", "b"),
      featureEvidence(openclawRepo, "readme", true),
      featureEvidence(openclawRepo, "license", true),
      featureEvidence(openclawRepo, "ci", false),
    ],
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