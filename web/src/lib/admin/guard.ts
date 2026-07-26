import { cookies } from "next/headers";
import type { NextRequest, NextResponse } from "next/server";
import { verifySessionToken } from "@/lib/admin/auth";
import {
  defaultDataDir,
  findLiveSession,
  getSessionSecret,
  SESSION_TTL_MS,
} from "@/lib/admin/store";
import { getSsoConfig, ssoEnabled, verifySsoAdmin, type SsoIdentity } from "@/lib/admin/sso";

// Session guard for /admin pages and route handlers. Cookie is httpOnly+signed; id must match a live server-side session so logout/expiry are enforced server-side. In SSO mode the IdP JWT cookie is verified server-side on every check; local sessions are bypassed for auth (store still supplies the panel slug).

export const SESSION_COOKIE = "assay_admin_session";

export interface AdminPrincipal {
  issuer: string;
  subject: string;
  displayName: string;
}

async function sessionIdFromToken(token: string | undefined): Promise<string | null> {
  if (!token) return null;
  const dir = defaultDataDir();
  const secret = await getSessionSecret(dir);
  if (!secret) return null;
  const sessionId = verifySessionToken(token, secret);
  if (!sessionId) return null;
  const live = await findLiveSession(dir, sessionId);
  return live ? sessionId : null;
}

// SSO branch: verify the IdP JWT and map identity onto the same opaque non-null id contract.
async function ssoAdminId(token: string | undefined): Promise<string | null> {
  const identity = await verifySsoAdmin(token);
  return identity ? `sso:${identity.subject}` : null;
}

// Server components/pages: reads the request cookie store.
export async function getAdminSessionId(): Promise<string | null> {
  const store = await cookies();
  if (ssoEnabled()) {
    return ssoAdminId(store.get(getSsoConfig()!.cookieName)?.value);
  }
  return sessionIdFromToken(store.get(SESSION_COOKIE)?.value);
}

// Route handlers: reads the cookie off the incoming request.
export async function requestSessionId(request: NextRequest): Promise<string | null> {
  if (ssoEnabled()) {
    return ssoAdminId(request.cookies.get(getSsoConfig()!.cookieName)?.value);
  }
  return sessionIdFromToken(request.cookies.get(SESSION_COOKIE)?.value);
}

/** Derives the reviewer identity on the server; callers never provide it. */
export async function requestAdminPrincipal(request: NextRequest): Promise<AdminPrincipal | null> {
  if (ssoEnabled()) {
    const config = getSsoConfig()!;
    const identity: SsoIdentity | null = await verifySsoAdmin(request.cookies.get(config.cookieName)?.value);
    return identity ? {
      issuer: config.issuer!, subject: identity.subject, displayName: identity.username,
    } : null;
  }
  const sessionId = await requestSessionId(request);
  if (!sessionId) return null;
  const admin = await import("@/lib/admin/store").then(({ getAdmin, defaultDataDir }) => getAdmin(defaultDataDir()));
  return admin ? { issuer: "assay-local", subject: admin.username, displayName: admin.username } : null;
}

function requestIsHttps(request: NextRequest): boolean {
  const forwarded = request.headers.get("x-forwarded-proto");
  if (forwarded) return forwarded.split(",")[0]!.trim() === "https";
  return request.nextUrl.protocol === "https:";
}

export function setSessionCookie(
  response: NextResponse,
  request: NextRequest,
  cookieValue: string,
): void {
  response.cookies.set(SESSION_COOKIE, cookieValue, {
    httpOnly: true,
    sameSite: "lax",
    // Secure follows the actual scheme so plain-HTTP self-hosted deployments still get a working signed session.
    secure: requestIsHttps(request),
    path: "/",
    maxAge: Math.floor(SESSION_TTL_MS / 1000),
  });
}

export function clearSessionCookie(
  response: NextResponse,
  request: NextRequest,
): void {
  response.cookies.set(SESSION_COOKIE, "", {
    httpOnly: true,
    sameSite: "lax",
    secure: requestIsHttps(request),
    path: "/",
    maxAge: 0,
  });
}
