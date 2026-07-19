// Admin store state shape, defaults, validation, and legacy migration. The
// storage adapter is a single JSON document under a data directory
// (ASSAY_DATA_DIR, defaulting to ./data) so a real backend can replace the
// persistence layer without touching routes or UI.

import path from "node:path";
import {
  generateSecret,
  newAdminSlug,
  newSetupToken,
} from "@/lib/admin/auth";

export interface AdminAccount {
  username: string;
  passwordHash: string;
  createdAt: string;
}

export interface SessionRecord {
  id: string;
  createdAt: string;
  expiresAt: string;
}

export interface AdminState {
  version: 1;
  admin: AdminAccount | null;
  sessionSecret: string;
  // Secret per-deployment URL slug: the admin area lives under /panel-<slug>.
  // Operator recovery: read this file (admin.json in the data dir) server-side.
  adminSlug: string;
  // One-time first-run setup token; non-null only while no admin exists.
  // Consumed (set to null) the moment setup succeeds.
  setupToken: string | null;
  sessions: SessionRecord[];
  hiddenEntryIds: string[];
}

export const SESSION_TTL_MS = 7 * 24 * 60 * 60 * 1000;

export const STORE_FILE = "admin.json";

export function defaultDataDir(): string {
  return process.env.ASSAY_DATA_DIR ?? path.join(process.cwd(), "data");
}

export function storePath(dir: string): string {
  return path.join(dir, STORE_FILE);
}

export function emptyState(): AdminState {
  return {
    version: 1,
    admin: null,
    sessionSecret: generateSecret(),
    adminSlug: newAdminSlug(),
    setupToken: newSetupToken(),
    sessions: [],
    hiddenEntryIds: [],
  };
}

export function isValidState(value: unknown): value is AdminState {
  if (typeof value !== "object" || value === null) return false;
  const state = value as Partial<AdminState>;
  return (
    state.version === 1 &&
    typeof state.sessionSecret === "string" &&
    typeof state.adminSlug === "string" &&
    (state.setupToken === null || typeof state.setupToken === "string") &&
    Array.isArray(state.sessions) &&
    Array.isArray(state.hiddenEntryIds) &&
    (state.admin === null ||
      (typeof state.admin === "object" &&
        state.admin !== null &&
        typeof state.admin.username === "string" &&
        typeof state.admin.passwordHash === "string"))
  );
}

// Stores written before the slug/token hardening lack the two new fields;
// fill them in place so an existing deployment keeps its admin and sessions.
export function migrateLegacyState(value: unknown): boolean {
  if (typeof value !== "object" || value === null) return false;
  const state = value as Partial<AdminState>;
  let changed = false;
  if (typeof state.adminSlug !== "string") {
    state.adminSlug = newAdminSlug();
    changed = true;
  }
  if (state.setupToken === undefined) {
    state.setupToken = state.admin ? null : newSetupToken();
    changed = true;
  }
  return changed;
}