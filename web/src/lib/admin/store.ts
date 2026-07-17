import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  constantTimeEquals,
  generateSecret,
  newAdminSlug,
  newSessionId,
  newSetupToken,
  signSession,
} from "@/lib/admin/auth";

// Server-side persistence for the first-run admin flow. The storage adapter is
// a single JSON document under a data directory (ASSAY_DATA_DIR, defaulting to
// ./data) so a real backend can replace this file without touching routes or
// UI. Writes go through a temp file + rename so a crash never leaves a
// half-written store. Single-process semantics only — honest for the current
// standalone deployment.

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

const STORE_FILE = "admin.json";

export function defaultDataDir(): string {
  return process.env.ASSAY_DATA_DIR ?? path.join(process.cwd(), "data");
}

function storePath(dir: string): string {
  return path.join(dir, STORE_FILE);
}

function emptyState(): AdminState {
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

function isValidState(value: unknown): value is AdminState {
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
function migrateLegacyState(value: unknown): boolean {
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

async function load(dir: string): Promise<AdminState | null> {
  let raw: string;
  try {
    raw = await readFile(storePath(dir), "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
  const parsed: unknown = JSON.parse(raw);
  const migrated = migrateLegacyState(parsed);
  if (!isValidState(parsed)) {
    throw new Error(`Corrupt admin store at ${storePath(dir)}`);
  }
  if (migrated) await save(dir, parsed);
  return parsed;
}

async function save(dir: string, state: AdminState): Promise<void> {
  await mkdir(dir, { recursive: true });
  const target = storePath(dir);
  const temp = `${target}.${process.pid}.tmp`;
  await writeFile(temp, `${JSON.stringify(state, null, 2)}\n`, "utf8");
  await rename(temp, target);
}

// All read-modify-write cycles are chained through one promise so two
// concurrent requests cannot both observe "no admin yet" and both win a
// load → mutate → save race. Single-process semantics, like the store itself.
let writeChain: Promise<unknown> = Promise.resolve();

function serialized<T>(fn: () => Promise<T>): Promise<T> {
  const next = writeChain.then(fn, fn);
  writeChain = next.catch(() => undefined);
  return next;
}

export function loadOrInitState(dir: string): Promise<AdminState> {
  return serialized(async () => {
    const existing = await load(dir);
    if (existing) return existing;
    const fresh = emptyState();
    await save(dir, fresh);
    return fresh;
  });
}

function mutate<T>(dir: string, fn: (state: AdminState) => T): Promise<T> {
  return serialized(async () => {
    const state = (await load(dir)) ?? emptyState();
    const result = fn(state);
    await save(dir, state);
    return result;
  });
}

export async function isConfigured(dir: string): Promise<boolean> {
  const state = await load(dir);
  return state?.admin !== null && state?.admin !== undefined;
}

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
  const state = await load(dir);
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
  const state = await load(dir);
  if (!state) return null;
  const record = state.sessions.find((s) => s.id === sessionId);
  if (!record || record.expiresAt <= nowIso) return null;
  return record;
}

export async function getSessionSecret(dir: string): Promise<string | null> {
  const state = await load(dir);
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

export async function hiddenEntryIds(dir: string): Promise<string[]> {
  try {
    const state = await load(dir);
    return state?.hiddenEntryIds ?? [];
  } catch {
    // A broken store must never take the public catalog down with it.
    return [];
  }
}

export async function setEntryHidden(
  dir: string,
  entryId: string,
  hidden: boolean,
): Promise<string[]> {
  return mutate(dir, (state) => {
    const set = new Set(state.hiddenEntryIds);
    if (hidden) set.add(entryId);
    else set.delete(entryId);
    state.hiddenEntryIds = [...set].sort();
    return state.hiddenEntryIds;
  });
}
