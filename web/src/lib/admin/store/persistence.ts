// Atomic load/save and the single-process serialization queue. Writes go
// through a temp file + rename so a crash never leaves a half-written store.
// Single-process semantics only — honest for the current standalone deployment.

import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import {
  emptyState,
  isValidState,
  migrateLegacyState,
  storePath,
  type AdminState,
} from "./state";

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

export function mutate<T>(dir: string, fn: (state: AdminState) => T): Promise<T> {
  return serialized(async () => {
    const state = (await load(dir)) ?? emptyState();
    const result = fn(state);
    await save(dir, state);
    return result;
  });
}

// Internal load exposed for read-only selectors that do not need serialization.
export async function readState(dir: string): Promise<AdminState | null> {
  return load(dir);
}