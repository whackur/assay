// Public re-export barrel for the versioned Assay JSON contracts under
// schemas/. Splits live in ./types/* by schema; this file preserves the
// "@/lib/contract/types" import path every consumer already uses.

export * from "./types/common";
export * from "./types/project-evaluation";
export * from "./types/project-comparison";
export * from "./types/project-evidence";