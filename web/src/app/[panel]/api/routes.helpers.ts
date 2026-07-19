// Shared helpers for the route-level contract tests under
// src/app/[panel]/api/*.test.ts. Each test invokes the real route handlers
// with real NextRequest objects against a fresh ASSAY_DATA_DIR, exactly as
// Next calls them. The helpers stay framework-agnostic so any scenario file
// can compose setup, login, and catalog toggles without duplicating wiring.

import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { NextRequest } from "next/server";
import { getBootstrap } from "@/lib/admin/store";

export const SESSION_COOKIE = "assay_admin_session";
export const USERNAME = "operator";
export const PASSWORD = "correct horse battery staple";
export const ENTRY_ID = "hermes-agent/hermes";
export const WRONG_PANEL = "panel-0000000000000000";

export async function freshDataDir(): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), "assay-admin-routes-"));
  process.env.ASSAY_DATA_DIR = dir;
  return dir;
}

export function routeContext(panel: string): { params: Promise<{ panel: string }> } {
  return { params: Promise.resolve({ panel }) };
}

export function jsonRequest(
  pathname: string,
  body: unknown,
  cookie?: string,
): NextRequest {
  return new NextRequest(`http://localhost${pathname}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(cookie ? { cookie } : {}),
    },
    body: JSON.stringify(body),
  });
}

export function formRequest(
  pathname: string,
  fields: Record<string, string>,
  cookie?: string,
): NextRequest {
  return new NextRequest(`http://localhost${pathname}`, {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded",
      ...(cookie ? { cookie } : {}),
    },
    body: new URLSearchParams(fields).toString(),
  });
}

// notFound() escapes a handler as Next's HTTP-fallback error; matching on the
// digest distinguishes a genuine 404 from any other rejection.
function isNotFoundError(error: unknown): boolean {
  const digest =
    typeof error === "object" && error !== null && "digest" in error
      ? String((error as { digest: unknown }).digest)
      : "";
  return (
    digest === "NEXT_NOT_FOUND" ||
    digest.startsWith("NEXT_HTTP_ERROR_FALLBACK;404")
  );
}

export function assertNotFound(work: Promise<unknown>): Promise<void> {
  return assert.rejects(work, (error: unknown) => isNotFoundError(error));
}

export async function panelSegment(dir: string): Promise<string> {
  const bootstrap = await getBootstrap(dir);
  return `panel-${bootstrap.adminSlug}`;
}

import { POST as setupPost } from "@/app/[panel]/api/setup/route";

export async function completeSetup(
  dir: string,
): Promise<{ panel: string; cookie: string }> {
  const panel = await panelSegment(dir);
  const { setupToken } = await getBootstrap(dir);
  const response = await setupPost(
    jsonRequest(`/${panel}/api/setup`, {
      token: setupToken,
      username: USERNAME,
      password: PASSWORD,
    }),
    routeContext(panel),
  );
  assert.equal(response.status, 201);
  const cookie = response.cookies.get(SESSION_COOKIE);
  assert.ok(cookie);
  return { panel, cookie: `${SESSION_COOKIE}=${cookie.value}` };
}

// With ASSAY_SSO_JWKS_URL set, the local credential surface disappears. The env
// flip is wrapped in try/finally so standalone-mode tests stay unaffected.
export async function inSsoMode(fn: () => Promise<void>): Promise<void> {
  process.env.ASSAY_SSO_JWKS_URL = "https://idp.example.test/jwks.json";
  process.env.ASSAY_SSO_ISSUER = "https://idp.example.test";
  try {
    await fn();
  } finally {
    delete process.env.ASSAY_SSO_JWKS_URL;
    delete process.env.ASSAY_SSO_ISSUER;
  }
}