// Admin setup and session actions: claiming the first-run admin, issuing and
// revoking sessions, and the bootstrap info used by the console banner and
// slug validation.

import {
  constantTimeEquals,
  newSessionId,
  signSession,
} from "@/lib/admin/auth";
import { loadOrInitState, mutate, readState } from "./persistence";
import {
  SESSION_TTL_MS,
  type AdminAccount,
  type AdminState,
  type SessionRecord,
} from "./state";

export type ClaimAdminResult =
  | { ok: true; admin: AdminAccount }
  | { ok: false; error: "already_configured" | "invalid_token" };

// The only way to create the admin account: present the one-time setup token.
// Token check, account creation, and token consumption happen inside a single
// serialized mutation, so concurrent claims cannot both succeed and the token
// is dead the moment setup completes.
export async function claimAdmin(
  dir: string,
  setupToken: string,
  username: string,
  passwordHash: string,
  nowIso = new Date().toISOString(),
): Promise<ClaimAdminResult> {
  return mutate(dir, (state): ClaimAdminResult => {
    if (state.admin) return { ok: false, error: "already_configured" };
    if (
      state.setupToken === null ||
      !constantTimeEquals(setupToken, state.setupToken)
    ) {
      return { ok: false, error: "invalid_token" };
    }
    state.admin = { username, passwordHash, createdAt: nowIso };
    state.setupToken = null;
    return { ok: true, admin: state.admin };
  });
}

export interface BootstrapInfo {
  configured: boolean;
  adminSlug: string;
  setupToken: string | null;
}

// Slug + token for the first-boot console banner and slug validation.
// Initializes the store on first boot so the banner can print real values.
export async function getBootstrap(dir: string): Promise<BootstrapInfo> {
  const state = await loadOrInitState(dir);
  return {
    configured: state.admin !== null,
    adminSlug: state.adminSlug,
    setupToken: state.setupToken,
  };
}

export async function getAdmin(dir: string): Promise<AdminAccount | null> {
  const state = await readState(dir);
  return state?.admin ?? null;
}

function pruneExpired(state: AdminState, nowIso: string): void {
  state.sessions = state.sessions.filter((s) => s.expiresAt > nowIso);
}

export interface IssuedSession {
  record: SessionRecord;
  cookieValue: string;
}

export async function createSession(
  dir: string,
  nowIso = new Date().toISOString(),
): Promise<IssuedSession> {
  return mutate(dir, (state): IssuedSession => {
    pruneExpired(state, nowIso);
    const record: SessionRecord = {
      id: newSessionId(),
      createdAt: nowIso,
      expiresAt: new Date(Date.parse(nowIso) + SESSION_TTL_MS).toISOString(),
    };
    state.sessions.push(record);
    return { record, cookieValue: signSession(record.id, state.sessionSecret) };
  });
}

export async function findLiveSession(
  dir: string,
  sessionId: string,
  nowIso = new Date().toISOString(),
): Promise<SessionRecord | null> {
  const state = await readState(dir);
  if (!state) return null;
  const record = state.sessions.find((s) => s.id === sessionId);
  if (!record || record.expiresAt <= nowIso) return null;
  return record;
}

export async function getSessionSecret(dir: string): Promise<string | null> {
  const state = await readState(dir);
  return state?.sessionSecret ?? null;
}

export async function removeSession(
  dir: string,
  sessionId: string,
): Promise<void> {
  await mutate(dir, (state) => {
    state.sessions = state.sessions.filter((s) => s.id !== sessionId);
  });
}