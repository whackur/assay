import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { NextRequest } from "next/server";
import { POST as setupPost } from "@/app/[panel]/api/setup/route";
import { POST as loginPost } from "@/app/[panel]/api/login/route";
import { POST as logoutPost } from "@/app/[panel]/api/logout/route";
import { POST as catalogPost } from "@/app/[panel]/api/catalog/route";
import { generateMetadata } from "@/app/[panel]/page";
import { getBootstrap, hiddenEntryIds } from "@/lib/admin/store";

// Route-level contract tests for the composed admin HTTP surface: the actual
// handlers from src/app/[panel]/api/*/route.ts invoked with real NextRequest
// objects against a fresh ASSAY_DATA_DIR per test, exactly as Next calls them.
// Gap: the AdminPage/setup/login page bodies need Next's request scope
// (next/headers cookies()), so page coverage stops at generateMetadata's slug
// guard; the session redirect inside the page body is not executable here.

const SESSION_COOKIE = "assay_admin_session";
const USERNAME = "operator";
const PASSWORD = "correct horse battery staple";
const ENTRY_ID = "hermes-agent/hermes";
const WRONG_PANEL = "panel-0000000000000000";

async function freshDataDir(): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), "assay-admin-routes-"));
  process.env.ASSAY_DATA_DIR = dir;
  return dir;
}

function routeContext(panel: string): { params: Promise<{ panel: string }> } {
  return { params: Promise.resolve({ panel }) };
}

function jsonRequest(
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

function formRequest(
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

function assertNotFound(work: Promise<unknown>): Promise<void> {
  return assert.rejects(work, (error: unknown) => isNotFoundError(error));
}

async function panelSegment(dir: string): Promise<string> {
  const bootstrap = await getBootstrap(dir);
  return `panel-${bootstrap.adminSlug}`;
}

async function completeSetup(
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

test("every admin route is a plain 404 under a wrong panel slug", async () => {
  await freshDataDir();
  await assertNotFound(
    setupPost(
      jsonRequest(`/${WRONG_PANEL}/api/setup`, {
        token: "anything",
        username: USERNAME,
        password: PASSWORD,
      }),
      routeContext(WRONG_PANEL),
    ),
  );
  await assertNotFound(
    loginPost(
      jsonRequest(`/${WRONG_PANEL}/api/login`, {
        username: USERNAME,
        password: PASSWORD,
      }),
      routeContext(WRONG_PANEL),
    ),
  );
  await assertNotFound(
    logoutPost(
      formRequest(`/${WRONG_PANEL}/api/logout`, {}),
      routeContext(WRONG_PANEL),
    ),
  );
  await assertNotFound(
    catalogPost(
      formRequest(`/${WRONG_PANEL}/api/catalog`, {
        entryId: ENTRY_ID,
        hidden: "true",
      }),
      routeContext(WRONG_PANEL),
    ),
  );
  await assertNotFound(
    generateMetadata({ params: Promise.resolve({ panel: WRONG_PANEL }) }),
  );
});

test("setup with a wrong or missing token is the same plain 404", async () => {
  const dir = await freshDataDir();
  const panel = await panelSegment(dir);
  await assertNotFound(
    setupPost(
      jsonRequest(`/${panel}/api/setup`, {
        token: "not-the-real-token",
        username: USERNAME,
        password: PASSWORD,
      }),
      routeContext(panel),
    ),
  );
  await assertNotFound(
    setupPost(
      jsonRequest(`/${panel}/api/setup`, {
        username: USERNAME,
        password: PASSWORD,
      }),
      routeContext(panel),
    ),
  );
  assert.equal((await getBootstrap(dir)).configured, false);
});

test("successful setup consumes the token and sets an httpOnly session cookie", async () => {
  const dir = await freshDataDir();
  const panel = await panelSegment(dir);
  const { setupToken } = await getBootstrap(dir);
  assert.ok(setupToken);

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
  assert.equal(cookie.httpOnly, true);
  assert.ok(cookie.value.length > 20);

  const after = await getBootstrap(dir);
  assert.equal(after.configured, true);
  assert.equal(after.setupToken, null);

  // A second setup attempt with the consumed token is a plain 404.
  await assertNotFound(
    setupPost(
      jsonRequest(`/${panel}/api/setup`, {
        token: setupToken,
        username: "intruder",
        password: PASSWORD,
      }),
      routeContext(panel),
    ),
  );
});

test("login before setup points the token holder at setup instead", async () => {
  const dir = await freshDataDir();
  const panel = await panelSegment(dir);
  const response = await loginPost(
    jsonRequest(`/${panel}/api/login`, {
      username: USERNAME,
      password: PASSWORD,
    }),
    routeContext(panel),
  );
  assert.equal(response.status, 409);
  assert.equal(response.cookies.get(SESSION_COOKIE), undefined);
});

test("login rejects a wrong password without issuing a session", async () => {
  const dir = await freshDataDir();
  const { panel } = await completeSetup(dir);
  const response = await loginPost(
    jsonRequest(`/${panel}/api/login`, {
      username: USERNAME,
      password: "definitely-not-the-password",
    }),
    routeContext(panel),
  );
  assert.equal(response.status, 401);
  assert.equal(response.cookies.get(SESSION_COOKIE), undefined);
});

test("login issues a session cookie the catalog route accepts", async () => {
  const dir = await freshDataDir();
  const { panel } = await completeSetup(dir);

  const login = await loginPost(
    jsonRequest(`/${panel}/api/login`, {
      username: USERNAME,
      password: PASSWORD,
    }),
    routeContext(panel),
  );
  assert.equal(login.status, 200);
  const cookie = login.cookies.get(SESSION_COOKIE);
  assert.ok(cookie);
  assert.equal(cookie.httpOnly, true);

  const toggle = await catalogPost(
    formRequest(
      `/${panel}/api/catalog`,
      { entryId: ENTRY_ID, hidden: "true" },
      `${SESSION_COOKIE}=${cookie.value}`,
    ),
    routeContext(panel),
  );
  assert.equal(toggle.status, 303);
  assert.deepEqual(await hiddenEntryIds(dir), [ENTRY_ID]);
});

test("catalog visibility toggle requires a valid session", async () => {
  const dir = await freshDataDir();
  const { panel } = await completeSetup(dir);

  const anonymous = await catalogPost(
    formRequest(`/${panel}/api/catalog`, { entryId: ENTRY_ID, hidden: "true" }),
    routeContext(panel),
  );
  assert.equal(anonymous.status, 401);

  const forged = await catalogPost(
    formRequest(
      `/${panel}/api/catalog`,
      { entryId: ENTRY_ID, hidden: "true" },
      `${SESSION_COOKIE}=forged.c29tZS1mb3JnZWQtc2lnbmF0dXJl`,
    ),
    routeContext(panel),
  );
  assert.equal(forged.status, 401);

  assert.deepEqual(await hiddenEntryIds(dir), []);
});

test("logout clears the cookie and invalidates the session server-side", async () => {
  const dir = await freshDataDir();
  const { panel, cookie } = await completeSetup(dir);

  const logout = await logoutPost(
    formRequest(`/${panel}/api/logout`, {}, cookie),
    routeContext(panel),
  );
  assert.equal(logout.status, 303);
  assert.equal(
    logout.headers.get("location"),
    `http://localhost/${panel}/login`,
  );
  const cleared = logout.cookies.get(SESSION_COOKIE);
  assert.ok(cleared);
  assert.equal(cleared.value, "");

  // The old cookie is dead server-side, not just cleared client-side.
  const afterward = await catalogPost(
    formRequest(
      `/${panel}/api/catalog`,
      { entryId: ENTRY_ID, hidden: "true" },
      cookie,
    ),
    routeContext(panel),
  );
  assert.equal(afterward.status, 401);
});
