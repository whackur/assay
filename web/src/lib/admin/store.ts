// Public re-export barrel for the admin store. Splits live in ./store/* by
// responsibility (state shape, persistence, actions, catalog selectors);
// this file preserves the "@/lib/admin/store" import path every consumer
// already uses.

export {
  defaultDataDir,
  emptyState,
  isValidState,
  migrateLegacyState,
  storePath,
  STORE_FILE,
  SESSION_TTL_MS,
  type AdminAccount,
  type AdminState,
  type SessionRecord,
} from "./store/state";
export { loadOrInitState, mutate, readState } from "./store/persistence";
export {
  claimAdmin,
  createSession,
  findLiveSession,
  getAdmin,
  getBootstrap,
  getSessionSecret,
  removeSession,
  type BootstrapInfo,
  type ClaimAdminResult,
  type IssuedSession,
} from "./store/actions";
export {
  hiddenEntryIds,
  isConfigured,
  setEntryHidden,
} from "./store/catalog";