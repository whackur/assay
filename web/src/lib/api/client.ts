import type { HostedSource, ProjectEvaluation, ProjectEvidence } from "@/lib/contract/types";
import type { AnalysisStage } from "@/lib/state/stages";
import type { EvaluatorProfileKind } from "@/lib/state/cooldown";
import { parseGithubTarget } from "@/lib/state/github-url";
import { RECORDS, findRecordId } from "@/lib/api/fixtures";

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
  | { kind: "admitted"; id: string; canonicalUrl: string };

export interface AssayApi {
  submit(input: string): Promise<SubmissionOutcome>;
  getRecord(id: string): Promise<EvaluationRecord | null>;
}

// A known cache hit navigates to the existing result; anything else is admitted
// as a new asynchronous job (specification 12.1 steps 4, 7, and 8).
export const fixtureApi: AssayApi = {
  async submit(input) {
    const parsed = parseGithubTarget(input);
    if (!parsed.ok) return { kind: "invalid", error: parsed.error };

    const id = findRecordId(parsed.source);
    if (id && RECORDS[id]!.state === "complete") {
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
