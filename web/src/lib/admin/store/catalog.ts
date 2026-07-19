// Catalog visibility selectors and the hidden-entry toggle. A broken store
// never takes the public catalog down with it.

import { mutate, readState } from "./persistence";

export async function isConfigured(dir: string): Promise<boolean> {
  const state = await readState(dir);
  return state?.admin !== null && state?.admin !== undefined;
}

export async function hiddenEntryIds(dir: string): Promise<string[]> {
  try {
    const state = await readState(dir);
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