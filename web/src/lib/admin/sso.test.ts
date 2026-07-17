import assert from "node:assert/strict";
import { test } from "node:test";
import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT } from "jose";
import type { JWTVerifyGetKey } from "jose";
import {
  getSsoConfig,
  ssoEnabled,
  ssoLoginRedirect,
  verifySsoAdmin,
} from "@/lib/admin/sso";

// Unit tests for the SSO config parsing and JWT admin verification. JWT
// verification runs against a locally generated RSA keypair with jose's
// createLocalJWKSet injected in place of the remote JWKS fetch, so no network
// is involved and the exact same jwtVerify path is exercised.

const JWKS_URL = "https://idp.example.test/.well-known/jwks.json";
const ISSUER = "https://idp.example.test";

const SSO_ENV_KEYS = [
  "ASSAY_SSO_JWKS_URL",
  "ASSAY_SSO_ISSUER",
  "ASSAY_SSO_AUDIENCE",
  "ASSAY_SSO_COOKIE",
  "ASSAY_SSO_ADMIN_ROLE",
  "ASSAY_SSO_LOGIN_URL",
] as const;

// Every test mutates process.env; run it inside this wrapper so one test's
// mode never leaks into another (or into the standalone-mode suites).
async function withEnv(
  env: Partial<Record<(typeof SSO_ENV_KEYS)[number], string>>,
  fn: () => Promise<void> | void,
): Promise<void> {
  const saved = SSO_ENV_KEYS.map((key) => [key, process.env[key]] as const);
  for (const key of SSO_ENV_KEYS) delete process.env[key];
  Object.assign(process.env, env);
  try {
    await fn();
  } finally {
    for (const [key, value] of saved) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
}

interface TestIdp {
  getKey: JWTVerifyGetKey;
  sign: (
    claims: Record<string, unknown>,
    options?: { issuer?: string; audience?: string; expiresIn?: string },
  ) => Promise<string>;
}

async function makeIdp(): Promise<TestIdp> {
  const { privateKey, publicKey } = await generateKeyPair("RS256");
  const jwk = await exportJWK(publicKey);
  const getKey = createLocalJWKSet({ keys: [{ ...jwk, alg: "RS256" }] });
  return {
    getKey,
    sign: async (claims, options = {}) => {
      let jwt = new SignJWT(claims)
        .setProtectedHeader({ alg: "RS256" })
        .setIssuedAt()
        .setIssuer(options.issuer ?? ISSUER)
        .setExpirationTime(options.expiresIn ?? "10m");
      if (options.audience) jwt = jwt.setAudience(options.audience);
      return jwt.sign(privateKey);
    },
  };
}

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
