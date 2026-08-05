import { createRemoteJWKSet, jwtVerify } from "jose";
import type { JWTVerifyGetKey } from "jose";

// SSO mode: ASSAY_SSO_JWKS_URL switches admin auth to JWT verification; without it the first-run flow is unchanged. /panel-<slug> stays as defense in depth. Server-side only.

export interface SsoConfig {
  /** JWKS endpoint of the identity provider. Presence enables SSO mode. */
  jwksUrl: string;
  /** Required `iss` claim. SSO mode refuses to authenticate without it. */
  issuer: string | null;
  /** Optional `aud` claim; verified only when set. */
  audience: string | null;
  /** Cookie the IdP sets on the shared parent domain. */
  cookieName: string;
  /** Role (in the token's `roles` array claim) that grants admin access. */
  adminRole: string;
  /** IdP sign-in page; unauthenticated admin pages redirect here when set. */
  loginUrl: string | null;
}

export interface SsoIdentity {
  /** Token subject (`sub`), the stable IdP user id. */
  subject: string;
  /** Human-readable name when the token carries one, else the subject. */
  username: string;
  roles: string[];
}

// Re-read env per call so tests can flip modes; only the JWKS fetcher is cached.
export function getSsoConfig(): SsoConfig | null {
  const jwksUrl = process.env.ASSAY_SSO_JWKS_URL;
  if (!jwksUrl) return null;
  return {
    jwksUrl,
    issuer: process.env.ASSAY_SSO_ISSUER ?? null,
    audience: process.env.ASSAY_SSO_AUDIENCE ?? null,
    cookieName: process.env.ASSAY_SSO_COOKIE ?? "access_token",
    adminRole: process.env.ASSAY_SSO_ADMIN_ROLE ?? "admin",
    loginUrl: process.env.ASSAY_SSO_LOGIN_URL ?? null,
  };
}

export function ssoEnabled(): boolean {
  return getSsoConfig() !== null;
}

// Remote JWKS caches fetched keys internally, so it must survive across requests: module-level lazy singleton keyed by URL.
let cachedJwks: { url: string; getKey: JWTVerifyGetKey } | null = null;

function remoteJwks(url: string): JWTVerifyGetKey {
  if (cachedJwks?.url !== url) {
    cachedJwks = { url, getKey: createRemoteJWKSet(new URL(url)) };
  }
  return cachedJwks.getKey;
}

let warnedMissingIssuer = false;

// IdP hand-off URL for unauthenticated admin pages, or null when no login URL is configured (page then renders the same 404 as a wrong slug). Pure for unit testing.
export function ssoLoginRedirect(returnUrl: string): string | null {
  const loginUrl = getSsoConfig()?.loginUrl;
  if (!loginUrl) return null;
  const target = new URL(loginUrl);
  target.searchParams.set("returnUrl", returnUrl);
  return target.toString();
}

// Unreachable JWKS = no admin can sign in; throttle to avoid log flood from a broken IdP.
const JWKS_WARNING_INTERVAL_MS = 5 * 60 * 1000;
let lastJwksWarningAt = 0;

// jose ERR_JWT*/ERR_JWS*/no-matching-key codes are token-shaped failures; anything else is a JWKS endpoint resolution problem.
function isTokenError(error: unknown): boolean {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : "";
  return (
    code.startsWith("ERR_JWT") ||
    code.startsWith("ERR_JWS") ||
    code === "ERR_JWKS_NO_MATCHING_KEY" ||
    code === "ERR_JWKS_MULTIPLE_MATCHING_KEYS"
  );
}

function warnJwksUnreachable(url: string, error: unknown): void {
  const now = Date.now();
  if (now - lastJwksWarningAt < JWKS_WARNING_INTERVAL_MS) return;
  lastJwksWarningAt = now;
  console.error(
    `[assay] SSO JWKS endpoint ${url} is unreachable or invalid; admin sign-in will fail until it recovers:`,
    error,
  );
}

// Verifies the SSO cookie and authorizes the admin role. Returns identity on success, null on ANY failure (callers treat null as "not signed in"; never throws). `getKey` is injectable for tests.
export async function verifySsoAdmin(
  cookieValue: string | undefined,
  getKey?: JWTVerifyGetKey,
): Promise<SsoIdentity | null> {
  const config = getSsoConfig();
  if (!config || !cookieValue) return null;
  if (!config.issuer) {
    // Issuer pinning is mandatory; without it any tenant of the JWKS host would pass. Fail closed.
    if (!warnedMissingIssuer) {
      warnedMissingIssuer = true;
      console.error(
        "[assay] ASSAY_SSO_JWKS_URL is set but ASSAY_SSO_ISSUER is not; refusing all SSO logins.",
      );
    }
    return null;
  }

  try {
    const { payload } = await jwtVerify(
      cookieValue,
      getKey ?? remoteJwks(config.jwksUrl),
      {
        issuer: config.issuer,
        ...(config.audience ? { audience: config.audience } : {}),
        algorithms: ["RS256"],
      },
    );
    const roles = Array.isArray(payload.roles)
      ? payload.roles.filter((role): role is string => typeof role === "string")
      : [];
    if (!roles.includes(config.adminRole)) return null;
    const subject = typeof payload.sub === "string" ? payload.sub : null;
    if (!subject) return null;
    const username =
      typeof payload.username === "string"
        ? payload.username
        : typeof payload.preferred_username === "string"
          ? payload.preferred_username
          : subject;
    return { subject, username, roles };
  } catch (error) {
    // Unreachable JWKS locks every admin out, so that one case gets a throttled warning; other failures stay silent.
    if (!isTokenError(error)) {
      warnJwksUnreachable(config.jwksUrl, error);
    }
    return null;
  }
}
