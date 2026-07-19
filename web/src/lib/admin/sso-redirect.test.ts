// SSO redirect and JWKS-failure observability unit tests. The redirect
// decision lives in the pure ssoLoginRedirect helper (the page body is not
// executable under node:test); an unreachable JWKS warns the operator once
// while bad tokens stay silent.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { JWTVerifyGetKey } from "jose";
import { ssoLoginRedirect, verifySsoAdmin } from "@/lib/admin/sso";
import { ISSUER, JWKS_URL, makeIdp, withEnv } from "./sso.helpers";

// --- Unauthenticated-page redirect contract ---------------------------------
// [panel]/page.tsx cannot execute under node:test (its guard call needs Next's
// request-scoped cookies()), so the redirect decision lives in the pure
// ssoLoginRedirect helper the page calls: a configured login URL yields the
// IdP hand-off with returnUrl, and no login URL yields null (the page then
// renders notFound(), same as a wrong slug).

test("ssoLoginRedirect builds the IdP URL with a returnUrl param", async () => {
  await withEnv(
    {
      ASSAY_SSO_JWKS_URL: JWKS_URL,
      ASSAY_SSO_ISSUER: ISSUER,
      ASSAY_SSO_LOGIN_URL: "https://hakhub.net/login?theme=dark",
    },
    () => {
      const target = ssoLoginRedirect("/panel-abc123");
      assert.ok(target);
      const url = new URL(target);
      assert.equal(url.origin, "https://hakhub.net");
      assert.equal(url.pathname, "/login");
      // Existing query params survive; returnUrl is appended.
      assert.equal(url.searchParams.get("theme"), "dark");
      assert.equal(url.searchParams.get("returnUrl"), "/panel-abc123");
    },
  );
});

test("ssoLoginRedirect is null without ASSAY_SSO_LOGIN_URL (page 404s)", async () => {
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    () => {
      assert.equal(ssoLoginRedirect("/panel-abc123"), null);
    },
  );
  await withEnv({}, () => {
    assert.equal(ssoLoginRedirect("/panel-abc123"), null);
  });
});

// --- JWKS failure observability ----------------------------------------------

test("an unreachable JWKS warns the operator once; bad tokens stay silent", async (t) => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const errors: unknown[][] = [];
      t.mock.method(console, "error", (...args: unknown[]) => {
        errors.push(args);
      });

      // Token-shaped failures (expired, garbage) never log.
      const expired = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { expiresIn: "-10m" },
      );
      assert.equal(await verifySsoAdmin(expired, idp.getKey), null);
      assert.equal(await verifySsoAdmin("eyJhbGciOiJSUzI1NiJ9.broken.sig", idp.getKey), null);
      assert.equal(errors.length, 0);

      // A resolver failure (what a network/fetch error against the remote
      // JWKS surfaces as) fails closed AND warns, identifying the endpoint.
      const unreachable: JWTVerifyGetKey = () => {
        throw new TypeError("fetch failed");
      };
      const valid = await idp.sign({ sub: "user-42", roles: ["admin"] });
      assert.equal(await verifySsoAdmin(valid, unreachable), null);
      assert.equal(errors.length, 1);
      assert.match(String(errors[0]![0]), /JWKS endpoint/);
      assert.ok(String(errors[0]![0]).includes(JWKS_URL));

      // Throttled: an immediate second failure does not log again.
      assert.equal(await verifySsoAdmin(valid, unreachable), null);
      assert.equal(errors.length, 1);
    },
  );
});