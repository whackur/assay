// Shared fixture primitives: revision constants, source/score/evidence
// builders reused across every fixture category. Contract-based development
// and demo fixtures conform to schemas/project-evaluation/v1.json and
// schemas/project-evidence/v1.json. They carry no metric logic; scores are
// fixed authored values.

import type {
  HostedSource,
  ProjectEvidence,
  Score,
  Status,
} from "@/lib/contract/types";

export const REVISION = "0123456789abcdef0123456789abcdef01234567";
export const SNAPSHOT = "evidence:repository:snapshot";

export function score(value: number | null, confidence: number, status: Status = "complete"): Score {
  return {
    status,
    value,
    confidence,
    version: "project-score-1",
    evidence_ids: value === null ? [] : [SNAPSHOT],
  };
}

export function source(namespace: string, repository: string): HostedSource {
  return { kind: "hosted", provider: "github", namespace, repository };
}

export function snapshotEvidence(repo: HostedSource): ProjectEvidence {
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

export function fileEvidence(repo: HostedSource, path: string, grade: "a" | "b" | "c"): ProjectEvidence {
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

export function featureEvidence(repo: HostedSource, feature: string, present: boolean): ProjectEvidence {
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