import { cookies } from "next/headers";
import type { NextRequest, NextResponse } from "next/server";
import { verifySessionToken } from "@/lib/admin/auth";
import {
  defaultDataDir,
  findLiveSession,
  getSessionSecret,
  SESSION_TTL_MS,
} from "@/lib/admin/store";
import { getSsoConfig, ssoEnabled, verifySsoAdmin } from "@/lib/admin/sso";

// Session guard shared by the /admin pages and the admin route handlers. The
// cookie is httpOnly and HMAC-signed; the id inside it must still match a live
// server-side session record, so logout and expiry are enforced server-side.
//
// In SSO mode (ASSAY_SSO_JWKS_URL set) the local session machinery is bypassed
// entirely: the admin identity is the IdP's JWT cookie, verified server-side
// on every check. Local sessions and admin.json credentials are ignored for
// auth, though the store still supplies the secret panel slug.

export const SESSION_COOKIE = "assay_admin_session";

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

// SSO branch shared by both cookie sources: verify the IdP JWT and map the
// identity onto the same "opaque non-null id" contract callers already use.
async function ssoAdminId(token: string | undefined): Promise<string | null> {
  const identity = await verifySsoAdmin(token);
  return identity ? `sso:${identity.subject}` : null;
}

// For server components and pages: reads the request cookie store.
export async function getAdminSessionId(): Promise<string | null> {
  const store = await cookies();
  if (ssoEnabled()) {
    return ssoAdminId(store.get(getSsoConfig()!.cookieName)?.value);
  }
  return sessionIdFromToken(store.get(SESSION_COOKIE)?.value);
}

// For route handlers: reads the cookie off the incoming request.
export async function requestSessionId(request: NextRequest): Promise<string | null> {
  if (ssoEnabled()) {
    return ssoAdminId(request.cookies.get(getSsoConfig()!.cookieName)?.value);
  }
  return sessionIdFromToken(request.cookies.get(SESSION_COOKIE)?.value);
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
    // Secure follows the actual scheme so a plain-HTTP self-hosted deployment
    // (the compose default) still gets a working, httpOnly, signed session.
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
