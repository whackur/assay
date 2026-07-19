// SSO config-parsing unit tests: env-driven mode selection, defaults, and
// explicit overrides. JWT verification runs against a locally generated RSA
// keypair (see sso.helpers.ts).

import assert from "node:assert/strict";
import { test } from "node:test";
import { getSsoConfig, ssoEnabled } from "@/lib/admin/sso";
import { ISSUER, JWKS_URL, withEnv } from "./sso.helpers";

test("SSO is disabled when ASSAY_SSO_JWKS_URL is unset", async () => {
  await withEnv({}, () => {
    assert.equal(ssoEnabled(), false);
    assert.equal(getSsoConfig(), null);
  });
});

test("config parsing applies defaults for cookie name and admin role", async () => {
  await withEnv({ ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER }, () => {
    assert.equal(ssoEnabled(), true);
    const config = getSsoConfig();
    assert.ok(config);
    assert.equal(config.jwksUrl, JWKS_URL);
    assert.equal(config.issuer, ISSUER);
    assert.equal(config.audience, null);
    assert.equal(config.cookieName, "access_token");
    assert.equal(config.adminRole, "admin");
    assert.equal(config.loginUrl, null);
  });
});

test("config parsing honors explicit overrides", async () => {
  await withEnv(
    {
      ASSAY_SSO_JWKS_URL: JWKS_URL,
      ASSAY_SSO_ISSUER: ISSUER,
      ASSAY_SSO_AUDIENCE: "assay",
      ASSAY_SSO_COOKIE: "hakhub_token",
      ASSAY_SSO_ADMIN_ROLE: "superuser",
      ASSAY_SSO_LOGIN_URL: "https://hakhub.net/login",
    },
    () => {
      const config = getSsoConfig();
      assert.ok(config);
      assert.equal(config.audience, "assay");
      assert.equal(config.cookieName, "hakhub_token");
      assert.equal(config.adminRole, "superuser");
      assert.equal(config.loginUrl, "https://hakhub.net/login");
    },
  );
});