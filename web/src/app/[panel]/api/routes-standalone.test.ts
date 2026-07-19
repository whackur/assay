// Route-level contract tests for setup and login in standalone mode.
// SSO-mode scenarios live in routes-sso.test.ts; shared helpers live in
// routes.helpers.ts. Catalog and logout tests live in
// routes-standalone-session.test.ts.

import assert from "node:assert/strict";
import { test } from "node:test";
import { POST as setupPost } from "@/app/[panel]/api/setup/route";
import { POST as loginPost } from "@/app/[panel]/api/login/route";
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