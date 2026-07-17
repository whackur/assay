import {
  createHash,
  createHmac,
  randomBytes,
  scrypt,
  timingSafeEqual,
} from "node:crypto";

// Pure credential and session-token primitives for the first-run admin flow.
// Passwords are hashed with Node's built-in scrypt (no external dependency);
// session tokens are an opaque random id signed with a server-side HMAC secret
// so a cookie value cannot be forged or guessed. No Next.js imports here so
// the module stays unit-testable under node:test.

const SCRYPT_N = 16384;
const SCRYPT_R = 8;
const SCRYPT_P = 1;
const KEY_LENGTH = 32;
const SALT_LENGTH = 16;

function scryptAsync(
  password: string,
  salt: Buffer,
  options: { N: number; r: number; p: number },
): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    scrypt(password, salt, KEY_LENGTH, options, (error, derived) => {
      if (error) reject(error);
      else resolve(derived);
    });
  });
}

export async function hashPassword(password: string): Promise<string> {
  const salt = randomBytes(SALT_LENGTH);
  const derived = await scryptAsync(password, salt, {
    N: SCRYPT_N,
    r: SCRYPT_R,
    p: SCRYPT_P,
  });
  return [
    "scrypt",
    String(SCRYPT_N),
    String(SCRYPT_R),
    String(SCRYPT_P),
    salt.toString("base64url"),
    derived.toString("base64url"),
  ].join("$");
}

export async function verifyPassword(
  password: string,
  encoded: string,
): Promise<boolean> {
  const parts = encoded.split("$");
  if (parts.length !== 6 || parts[0] !== "scrypt") return false;
  const [, nText, rText, pText, saltText, hashText] = parts;
  const n = Number(nText);
  const r = Number(rText);
  const p = Number(pText);
  if (!Number.isInteger(n) || !Number.isInteger(r) || !Number.isInteger(p)) {
    return false;
  }
  const salt = Buffer.from(saltText!, "base64url");
  const expected = Buffer.from(hashText!, "base64url");
  if (expected.length !== KEY_LENGTH) return false;
  const derived = await scryptAsync(password, salt, { N: n, r, p });
  return timingSafeEqual(derived, expected);
}

export function generateSecret(): string {
  return randomBytes(32).toString("base64url");
}

export function newSessionId(): string {
  return randomBytes(18).toString("base64url");
}

// Per-deployment secret URL slug for the admin panel. 12 random bytes encode
// to exactly 16 URL-safe base64url characters (~96 bits) — a capability
// secret layered on top of, never instead of, session authentication.
export function newAdminSlug(): string {
  return randomBytes(12).toString("base64url");
}

// One-time first-run setup token (Jenkins initialAdminPassword pattern).
// Printed to the server console on boot and consumed when setup succeeds.
export function newSetupToken(): string {
  return randomBytes(24).toString("base64url");
}

// Constant-time string equality for URL-carried secrets (slug, setup token).
// Hashing both sides first hides length differences from the comparison.
export function constantTimeEquals(a: string, b: string): boolean {
  const digestA = createHash("sha256").update(a).digest();
  const digestB = createHash("sha256").update(b).digest();
  return timingSafeEqual(digestA, digestB);
}

function signature(sessionId: string, secret: string): Buffer {
  return createHmac("sha256", secret).update(sessionId).digest();
}

// Cookie value: "<session id>.<hmac-sha256(session id)>", both base64url.
export function signSession(sessionId: string, secret: string): string {
  return `${sessionId}.${signature(sessionId, secret).toString("base64url")}`;
}

export function verifySessionToken(
  token: string,
  secret: string,
): string | null {
  const dot = token.lastIndexOf(".");
  if (dot <= 0) return null;
  const sessionId = token.slice(0, dot);
  let provided: Buffer;
  try {
    provided = Buffer.from(token.slice(dot + 1), "base64url");
  } catch {
    return null;
  }
  const expected = signature(sessionId, secret);
  if (provided.length !== expected.length) return null;
  return timingSafeEqual(provided, expected) ? sessionId : null;
}

const USERNAME_PATTERN = /^[a-zA-Z0-9](?:[a-zA-Z0-9._-]{1,30})[a-zA-Z0-9]$/;

export function validateUsername(username: string): string | null {
  if (username.length < 3 || username.length > 32) {
    return "Username must be between 3 and 32 characters.";
  }
  if (!USERNAME_PATTERN.test(username)) {
    return "Username may contain letters, digits, dots, underscores, and hyphens, and must start and end with a letter or digit.";
  }
  return null;
}

export function validatePassword(password: string): string | null {
  if (password.length < 10) {
    return "Password must be at least 10 characters.";
  }
  if (password.length > 256) {
    return "Password must be at most 256 characters.";
  }
  return null;
}
