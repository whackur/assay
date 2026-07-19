// Shared helpers for the SSO unit-test suites under src/lib/admin/sso*.test.ts.
// JWT verification runs against a locally generated RSA keypair with jose's
// createLocalJWKSet injected in place of the remote JWKS fetch, so no network
// is involved and the exact same jwtVerify path is exercised.

import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT } from "jose";
import type { JWTVerifyGetKey } from "jose";

export const JWKS_URL = "https://idp.example.test/.well-known/jwks.json";
export const ISSUER = "https://idp.example.test";

export const SSO_ENV_KEYS = [
  "ASSAY_SSO_JWKS_URL",
  "ASSAY_SSO_ISSUER",
  "ASSAY_SSO_AUDIENCE",
  "ASSAY_SSO_COOKIE",
  "ASSAY_SSO_ADMIN_ROLE",
  "ASSAY_SSO_LOGIN_URL",
] as const;

// Every test mutates process.env; run it inside this wrapper so one test's
// mode never leaks into another (or into the standalone-mode suites).
export async function withEnv(
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

export interface TestIdp {
  getKey: JWTVerifyGetKey;
  sign: (
    claims: Record<string, unknown>,
    options?: { issuer?: string; audience?: string; expiresIn?: string },
  ) => Promise<string>;
}

export async function makeIdp(): Promise<TestIdp> {
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