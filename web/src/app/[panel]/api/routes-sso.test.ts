// Route-level contract tests for SSO mode: with ASSAY_SSO_JWKS_URL set, the
// local credential surface disappears (setup, login, logout are plain 404s
// even under the correct panel slug) and local session cookies stop
// authenticating API routes. The end-to-end JWT path exercises the production
// createRemoteJWKSet wiring against a real local HTTP JWKS endpoint.

import assert from "node:assert/strict";
import { test } from "node:test";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { exportJWK, generateKeyPair, SignJWT } from "jose";
import { POST as setupPost } from "@/app/[panel]/api/setup/route";
import { POST as loginPost } from "@/app/[panel]/api/login/route";
import { POST as logoutPost } from "@/app/[panel]/api/logout/route";
import { POST as catalogPost } from "@/app/[panel]/api/catalog/route";
import { getBootstrap, hiddenEntryIds } from "@/lib/admin/store";
import {
  ENTRY_ID,
  PASSWORD,
  USERNAME,
  WRONG_PANEL,
  assertNotFound,
  completeSetup,
  formRequest,
  freshDataDir,
  inSsoMode,
  jsonRequest,
  panelSegment,
  routeContext,
} from "./routes.helpers";

test("SSO mode disables setup, login, and logout even under the right slug", async () => {
  const dir = await freshDataDir();
  const panel = await panelSegment(dir);
  const { setupToken } = await getBootstrap(dir);
  await inSsoMode(async () => {
    await assertNotFound(
      setupPost(
        jsonRequest(`/${panel}/api/setup`, {
          token: setupToken,
          username: USERNAME,
          password: PASSWORD,
        }),
        routeContext(panel),
      ),
    );
    await assertNotFound(
      loginPost(
        jsonRequest(`/${panel}/api/login`, {
          username: USERNAME,
          password: PASSWORD,
        }),
        routeContext(panel),
      ),
    );
    await assertNotFound(
      logoutPost(formRequest(`/${panel}/api/logout`, {}), routeContext(panel)),
    );
  });
  // Back in standalone mode, the untouched setup token still works.
  await completeSetup(dir);
});

test("SSO mode ignores local session cookies on authenticated API routes", async () => {
  const dir = await freshDataDir();
  // A session minted in standalone mode must not authenticate in SSO mode.
  const { panel, cookie } = await completeSetup(dir);
  await inSsoMode(async () => {
    const response = await catalogPost(
      formRequest(
        `/${panel}/api/catalog`,
        { entryId: ENTRY_ID, hidden: "true" },
        cookie,
      ),
      routeContext(panel),
    );
    assert.equal(response.status, 401);
    assert.deepEqual(await hiddenEntryIds(dir), []);
  });
});

test("SSO mode keeps the wrong-slug 404 on API routes", async () => {
  await freshDataDir();
  await inSsoMode(async () => {
    await assertNotFound(
      catalogPost(
        formRequest(`/${WRONG_PANEL}/api/catalog`, {
          entryId: ENTRY_ID,
          hidden: "true",
        }),
        routeContext(WRONG_PANEL),
      ),
    );
  });
});

test("SSO mode grants admin access end-to-end with a valid JWT cookie", async () => {
  const dir = await freshDataDir();
  const panel = await panelSegment(dir);

  // A real IdP stand-in: a local RS256 keypair whose public JWK is served by
  // an actual HTTP endpoint, so the production createRemoteJWKSet path in
  // sso.ts (not an injected test resolver) fetches and verifies against it.
  const { privateKey, publicKey } = await generateKeyPair("RS256");
  const jwk = await exportJWK(publicKey);
  const server = createServer((_request, response) => {
    response.setHeader("content-type", "application/json");
    response.end(JSON.stringify({ keys: [{ ...jwk, alg: "RS256" }] }));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  const issuer = "https://idp.example.test";

  process.env.ASSAY_SSO_JWKS_URL = `http://127.0.0.1:${port}/jwks.json`;
  process.env.ASSAY_SSO_ISSUER = issuer;
  try {
    const token = await new SignJWT({ roles: ["member", "admin"] })
      .setProtectedHeader({ alg: "RS256" })
      .setSubject("user-42")
      .setIssuedAt()
      .setIssuer(issuer)
      .setExpirationTime("10m")
      .sign(privateKey);

    // The default SSO cookie authenticates the catalog toggle through the
    // real wiring: guard -> verifySsoAdmin -> remote JWKS fetch.
    const granted = await catalogPost(
      formRequest(
        `/${panel}/api/catalog`,
        { entryId: ENTRY_ID, hidden: "true" },
        `access_token=${token}`,
      ),
      routeContext(panel),
    );
    assert.equal(granted.status, 303);
    assert.deepEqual(await hiddenEntryIds(dir), [ENTRY_ID]);

    // Same wiring rejects a token without the admin role.
    const memberToken = await new SignJWT({ roles: ["member"] })
      .setProtectedHeader({ alg: "RS256" })
      .setSubject("user-7")
      .setIssuedAt()
      .setIssuer(issuer)
      .setExpirationTime("10m")
      .sign(privateKey);
    const denied = await catalogPost(
      formRequest(
        `/${panel}/api/catalog`,
        { entryId: ENTRY_ID, hidden: "false" },
        `access_token=${memberToken}`,
      ),
      routeContext(panel),
    );
    assert.equal(denied.status, 401);
    assert.deepEqual(await hiddenEntryIds(dir), [ENTRY_ID]);
  } finally {
    delete process.env.ASSAY_SSO_JWKS_URL;
    delete process.env.ASSAY_SSO_ISSUER;
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});