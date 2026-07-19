// TypeScript mirror of schemas/project-evidence/v1.json.

import type { RepositorySource, Status } from "./common";

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