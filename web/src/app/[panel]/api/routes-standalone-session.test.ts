// Route-level contract tests for catalog visibility and logout in standalone
// mode. Setup and login tests live in routes-standalone.test.ts; shared
// helpers live in routes.helpers.ts.

import assert from "node:assert/strict";
import { test } from "node:test";
import { POST as catalogPost } from "@/app/[panel]/api/catalog/route";
import { POST as logoutPost } from "@/app/[panel]/api/logout/route";
import { hiddenEntryIds } from "@/lib/admin/store";
import {
  ENTRY_ID,
  SESSION_COOKIE,
  completeSetup,
  formRequest,
  freshDataDir,
  routeContext,
} from "./routes.helpers";

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