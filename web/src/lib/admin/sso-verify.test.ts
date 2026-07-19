// SSO JWT verification unit tests: role check, custom role names, issuer
// pinning, expiry, audience verification, and fail-closed behavior on garbage
// tokens, missing cookie, and missing issuer. JWT verification runs against a
// locally generated RSA keypair (see sso.helpers.ts).

import assert from "node:assert/strict";
import { test } from "node:test";
import { verifySsoAdmin } from "@/lib/admin/sso";
import { ISSUER, JWKS_URL, makeIdp, withEnv } from "./sso.helpers";

test("accepts a valid token whose roles include the admin role", async () => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const token = await idp.sign({
        sub: "user-42",
        username: "operator",
        roles: ["member", "admin"],
      });
      const identity = await verifySsoAdmin(token, idp.getKey);
      assert.ok(identity);
      assert.equal(identity.subject, "user-42");
      assert.equal(identity.username, "operator");
      assert.deepEqual(identity.roles, ["member", "admin"]);
    },
  );
});

test("rejects a valid token that lacks the admin role", async () => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const token = await idp.sign({ sub: "user-42", roles: ["member"] });
      assert.equal(await verifySsoAdmin(token, idp.getKey), null);
    },
  );
});

test("honors a custom admin role name", async () => {
  const idp = await makeIdp();
  await withEnv(
    {
      ASSAY_SSO_JWKS_URL: JWKS_URL,
      ASSAY_SSO_ISSUER: ISSUER,
      ASSAY_SSO_ADMIN_ROLE: "superuser",
    },
    async () => {
      const admin = await idp.sign({ sub: "a", roles: ["superuser"] });
      const plain = await idp.sign({ sub: "b", roles: ["admin"] });
      assert.ok(await verifySsoAdmin(admin, idp.getKey));
      assert.equal(await verifySsoAdmin(plain, idp.getKey), null);
    },
  );
});

test("rejects a token with the wrong issuer", async () => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const token = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { issuer: "https://evil.example.test" },
      );
      assert.equal(await verifySsoAdmin(token, idp.getKey), null);
    },
  );
});

test("rejects an expired token", async () => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const token = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { expiresIn: "-10m" },
      );
      assert.equal(await verifySsoAdmin(token, idp.getKey), null);
    },
  );
});

test("verifies the audience only when ASSAY_SSO_AUDIENCE is set", async () => {
  const idp = await makeIdp();
  // Without a configured audience, a token carrying any (or no) audience passes.
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      const token = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { audience: "somewhere-else" },
      );
      assert.ok(await verifySsoAdmin(token, idp.getKey));
    },
  );
  // With one, only the matching audience passes.
  await withEnv(
    {
      ASSAY_SSO_JWKS_URL: JWKS_URL,
      ASSAY_SSO_ISSUER: ISSUER,
      ASSAY_SSO_AUDIENCE: "assay",
    },
    async () => {
      const good = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { audience: "assay" },
      );
      const bad = await idp.sign(
        { sub: "user-42", roles: ["admin"] },
        { audience: "somewhere-else" },
      );
      const none = await idp.sign({ sub: "user-42", roles: ["admin"] });
      assert.ok(await verifySsoAdmin(good, idp.getKey));
      assert.equal(await verifySsoAdmin(bad, idp.getKey), null);
      assert.equal(await verifySsoAdmin(none, idp.getKey), null);
    },
  );
});

test("fails closed on garbage tokens, missing cookie, and missing issuer", async () => {
  const idp = await makeIdp();
  await withEnv(
    { ASSAY_SSO_JWKS_URL: JWKS_URL, ASSAY_SSO_ISSUER: ISSUER },
    async () => {
      assert.equal(await verifySsoAdmin(undefined, idp.getKey), null);
      assert.equal(await verifySsoAdmin("", idp.getKey), null);
      assert.equal(await verifySsoAdmin("not-a-jwt", idp.getKey), null);
      // Missing subject is rejected even with the admin role present.
      const noSub = await idp.sign({ roles: ["admin"] });
      assert.equal(await verifySsoAdmin(noSub, idp.getKey), null);
    },
  );
  // Issuer pinning is mandatory: without ASSAY_SSO_ISSUER nobody authenticates.
  await withEnv({ ASSAY_SSO_JWKS_URL: JWKS_URL }, async () => {
    const token = await idp.sign({ sub: "user-42", roles: ["admin"] });
    assert.equal(await verifySsoAdmin(token, idp.getKey), null);
  });
  // Outside SSO mode the verifier authenticates nobody at all.
  await withEnv({}, async () => {
    const token = await idp.sign({ sub: "user-42", roles: ["admin"] });
    assert.equal(await verifySsoAdmin(token, idp.getKey), null);
  });
});