import type { HostedSource, ProjectEvaluation, ProjectEvidence } from "@/lib/contract/types";
import type { AnalysisStage } from "@/lib/state/stages";
import type { CooldownStatus, EvaluatorProfileKind } from "@/lib/state/cooldown";
import { cooldownStatus } from "@/lib/state/cooldown";
import { parseGithubTarget } from "@/lib/state/github-url";
import { RECORDS, SUBMISSION_COOLDOWNS, findRecordId } from "@/lib/api/fixtures";

// Thin client over the versioned Assay report contract. The repository has no
// hosted API yet, so this default implementation is fixture-backed. Swap the
// implementation for an HTTP transport without touching the UI once the Rust
// API exists. No business logic lives here.

export interface JobState {
  stage: AnalysisStage;
  started_at: string;
  canonical: HostedSource;
  canonical_url: string;
  revision: string;
  profile: EvaluatorProfileKind;
}

export type EvaluationRecord =
  | { id: string; state: "in_flight"; job: JobState }
  | {
      id: string;
      state: "complete";
      evaluation: ProjectEvaluation;
      evidence: ProjectEvidence[];
    };

export type SubmissionOutcome =
  | { kind: "invalid"; error: string }
  | { kind: "cached"; id: string; canonicalUrl: string }
  | { kind: "cooldown"; id: string; canonicalUrl: string; cooldown: CooldownStatus }
  | { kind: "admitted"; id: string; canonicalUrl: string };

export interface AssayApi {
  submit(input: string, nowIso?: string): Promise<SubmissionOutcome>;
  getRecord(id: string): Promise<EvaluationRecord | null>;
}

// A known cache hit navigates to the existing result. A recent run still inside
// its refresh cooldown reports the cooldown and next eligible time (spec 12.3
// and 12.5). Anything else is admitted as a new asynchronous job (spec 12.1).
export const fixtureApi: AssayApi = {
  async submit(input, nowIso = new Date().toISOString()) {
    const parsed = parseGithubTarget(input);
    if (!parsed.ok) return { kind: "invalid", error: parsed.error };

    const id = findRecordId(parsed.source);
    if (id && RECORDS[id]!.state === "complete") {
      const gate = SUBMISSION_COOLDOWNS[id];
      if (gate) {
        const cooldown = cooldownStatus(gate.profile, gate.last_run_at, nowIso);
        if (!cooldown.admitted) {
          return { kind: "cooldown", id, canonicalUrl: parsed.canonicalUrl, cooldown };
        }
      }
      return { kind: "cached", id, canonicalUrl: parsed.canonicalUrl };
    }
    return {
      kind: "admitted",
      id: id ?? "acme/in-progress",
      canonicalUrl: parsed.canonicalUrl,
    };
  },

  async getRecord(id) {
    return RECORDS[id] ?? null;
  },
};
