// Shared parser helpers used by every contract parser. Keeps the per-schema
// parse modules small and consistent.

export class ContractError extends Error {}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function require(condition: boolean, message: string): asserts condition {
  if (!condition) throw new ContractError(message);
}

export function asRecord(value: unknown, label: string): Record<string, unknown> {
  require(isRecord(value), `${label} must be an object`);
  return value as Record<string, unknown>;
}

const SCHEMA_VERSION_PATTERN = /^1\.\d+\.\d+$/;

export function requireSchemaVersion(
  value: Record<string, unknown>,
  label: string,
): void {
  require(
    typeof value.schema_version === "string" &&
      SCHEMA_VERSION_PATTERN.test(value.schema_version),
    `unsupported ${label} schema_version`,
  );
}

export function requireString(
  value: Record<string, unknown>,
  field: string,
  label: string,
): void {
  require(typeof value[field] === "string", `${label} ${field} is required`);
}

export function requireBoolean(
  value: Record<string, unknown>,
  field: string,
  label: string,
): void {
  require(typeof value[field] === "boolean", `${label} ${field} is required`);
}

export function requireArray(
  value: Record<string, unknown>,
  field: string,
  label: string,
): unknown[] {
  require(Array.isArray(value[field]), `${label} ${field} must be an array`);
  return value[field] as unknown[];
}

export function requireRecord(
  value: Record<string, unknown>,
  field: string,
  label: string,
): Record<string, unknown> {
  require(isRecord(value[field]), `${label} ${field} is required`);
  return value[field] as Record<string, unknown>;
}

const SHA256_PATTERN = /^sha256:[0-9a-f]{64}$/;
const REVISION_PATTERN = /^(?!(?:0{40}|0{64})$)(?:[0-9a-f]{40}|[0-9a-f]{64})$/;
const EVIDENCE_ID_PATTERN =
  /^evidence:[a-z0-9._-]+:[a-z0-9._-]+(?::[a-z0-9._-]+)*$/;
const VERSION_IDENTIFIER_PATTERN = /^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$/;
const MACHINE_CODE_PATTERN = /^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$/;

export function isSha256(value: unknown): value is string {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}

export function isRevision(value: unknown): value is string {
  return typeof value === "string" && REVISION_PATTERN.test(value);
}

export function isEvidenceId(value: unknown): value is string {
  return typeof value === "string" && EVIDENCE_ID_PATTERN.test(value);
}

export function isVersionIdentifier(value: unknown): value is string {
  return typeof value === "string" && VERSION_IDENTIFIER_PATTERN.test(value);
}

export function isMachineCode(value: unknown): value is string {
  return typeof value === "string" && MACHINE_CODE_PATTERN.test(value);
}

export function isTimestamp(value: unknown): value is string {
  return typeof value === "string" && /Z$/.test(value);
}