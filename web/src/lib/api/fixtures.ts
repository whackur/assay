// Public re-export barrel for contract-based development and demo fixtures. Splits live in ./fixtures/* by category (builders, evaluations, comparisons, records); this file preserves the "@/lib/api/fixtures" import path consumers use.

export { findRecordId, RECORDS, SUBMISSION_COOLDOWNS } from "./fixtures/records";
export type { CooldownFixture } from "./fixtures/records";
export { COMPARISONS } from "./fixtures/comparisons";