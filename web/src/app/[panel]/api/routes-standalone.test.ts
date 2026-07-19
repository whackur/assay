// Route-level contract tests for the composed admin HTTP surface in standalone
// (local credential) mode: the actual handlers from src/app/[panel]/api/*/route.ts
// invoked with real NextRequest objects against a fresh ASSAY_DATA_DIR per test.
// SSO-mode scenarios live in routes-sso.test.ts; shared helpers live in
// routes.helpers.ts.
//
// Gap: the AdminPage/setup/login page bodies need Next's request scope
// (next/headers cookies()), so page coverage stops at generateMetadata's slug
// guard; the session redirect inside the page body is not executable here.

import assert from "node:assert/strict";
import { test } from "node:test";
import { POST as setupPost } from "@/app/[panel]/api/setup/route";
import { POST as loginPost } from "@/app/[panel]/api/login/route";
import { POST as logoutPost } from "@/app/[panel]/api/logout/route";
import { POST as catalogPost } from "@/app/[panel]/api/catalog/route";
import { generateMetadata } from "@/app/[panel]/page";
import { getBootstrap, hiddenEntryIds } from "@/lib/admin/store";
import {
  ENTRY_ID,
  PASSWORD,
  SESSION_COOKIE,
  USERNAME,
  WRONG_PANEL,
  assertNotFound,
  completeSetup,
  formRequest,
  freshDataDir,
  jsonRequest,
  panelSegment,
  routeContext,
} from "./routes.helpers";

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