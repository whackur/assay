import { createRemoteJWKSet, jwtVerify } from "jose";
import type { JWTVerifyGetKey } from "jose";

// Optional SSO mode for deployments that trust an external identity provider
// (an RS256-signing OIDC-style issuer such as hakhub.net). The mode is
// selected purely by environment: setting ASSAY_SSO_JWKS_URL switches every
// admin auth check from local sessions to JWT verification against that JWKS.
// Without it, the standalone first-run flow (setup token, username/password,
// admin.json sessions) is completely unchanged.
//
// The secret /panel-<slug> path stays in force in both modes — it is defense
// in depth on top of, never instead of, real authentication. Verification is
// strictly server-side; nothing here may run in the browser.

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

// Config is re-read from the environment on every call (cheap string reads),
// so tests can flip modes per test; only the JWKS fetcher below is cached.
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

// The remote JWKS handle caches fetched keys internally, so it must survive
// across requests: a module-level lazy singleton keyed by the URL (which only
// changes in tests).
let cachedJwks: { url: string; getKey: JWTVerifyGetKey } | null = null;

function remoteJwks(url: string): JWTVerifyGetKey {
  if (cachedJwks?.url !== url) {
    cachedJwks = { url, getKey: createRemoteJWKSet(new URL(url)) };
  }
  return cachedJwks.getKey;
}

let warnedMissingIssuer = false;

// Builds the IdP hand-off URL for an unauthenticated admin page in SSO mode,
// or null when no login URL is configured — the page then renders the same
// 404 as a wrong slug. Pure so the redirect contract is unit-testable without
// Next's request scope.
export function ssoLoginRedirect(returnUrl: string): string | null {
  const loginUrl = getSsoConfig()?.loginUrl;
  if (!loginUrl) return null;
  const target = new URL(loginUrl);
  target.searchParams.set("returnUrl", returnUrl);
  return target.toString();
}

// A bad token is business as usual and stays silent, but an unreachable JWKS
// endpoint means NO admin can sign in — surface that to the operator, at most
// once per interval so a broken IdP does not flood the logs.
const JWKS_WARNING_INTERVAL_MS = 5 * 60 * 1000;
let lastJwksWarningAt = 0;

// Token-shaped failures (bad signature, expired, wrong claims, malformed JWT,
// no matching key) carry jose's ERR_JWT*/ERR_JWS*/no-matching-key codes.
// Anything else out of jwtVerify — JWKS timeout, invalid JWKS document, plain
// fetch/network failure — is a resolution problem with the endpoint itself.
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

// Verifies the SSO cookie and authorizes the admin role. Returns the identity
// on success and null on ANY failure — missing cookie, bad signature, expired
// token, wrong issuer/audience, missing role, unreachable JWKS. Callers treat
// null as "not signed in"; this function never throws.
//
// `getKey` is injectable so tests can substitute jose's createLocalJWKSet for
// the remote fetch; production callers omit it.
export async function verifySsoAdmin(
  cookieValue: string | undefined,
  getKey?: JWTVerifyGetKey,
): Promise<SsoIdentity | null> {
  const config = getSsoConfig();
  if (!config || !cookieValue) return null;
  if (!config.issuer) {
    // Issuer pinning is mandatory in SSO mode; without it any token from any
    // tenant of the JWKS host would pass. Fail closed (nobody authenticates)
    // rather than falling back to local sessions.
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
    // Every failure is "not signed in" to the caller — verification errors
    // never escape into a page render or route handler. But an unreachable
    // JWKS endpoint (unlike a merely bad token) locks every admin out, so
    // that one case gets a throttled operator warning.
    if (!isTokenError(error)) {
      warnJwksUnreachable(config.jwksUrl, error);
    }
    return null;
  }
}
