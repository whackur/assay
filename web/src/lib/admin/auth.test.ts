import assert from "node:assert/strict";
import { test } from "node:test";
import {
  constantTimeEquals,
  generateSecret,
  hashPassword,
  newAdminSlug,
  newSessionId,
  newSetupToken,
  signSession,
  validatePassword,
  validateUsername,
  verifyPassword,
  verifySessionToken,
} from "@/lib/admin/auth";

test("hashes and verifies a password", async () => {
  const encoded = await hashPassword("correct horse battery staple");
  assert.ok(encoded.startsWith("scrypt$"));
  assert.equal(await verifyPassword("correct horse battery staple", encoded), true);
});

test("rejects a wrong password", async () => {
  const encoded = await hashPassword("correct horse battery staple");
  assert.equal(await verifyPassword("wrong horse", encoded), false);
});

test("two hashes of the same password differ by salt", async () => {
  const a = await hashPassword("correct horse battery staple");
  const b = await hashPassword("correct horse battery staple");
  assert.notEqual(a, b);
});

test("rejects malformed encoded hashes without throwing", async () => {
  assert.equal(await verifyPassword("anything", "not-a-hash"), false);
  assert.equal(await verifyPassword("anything", "bcrypt$x$y$z$w$v"), false);
});

test("session token round-trips through sign and verify", () => {
  const secret = generateSecret();
  const id = newSessionId();
  const token = signSession(id, secret);
  assert.equal(verifySessionToken(token, secret), id);
});

test("rejects a tampered session token", () => {
  const secret = generateSecret();
  const token = signSession(newSessionId(), secret);
  const tampered = `x${token.slice(1)}`;
  assert.equal(verifySessionToken(tampered, secret), null);
});

test("rejects a token signed with another secret", () => {
  const token = signSession(newSessionId(), generateSecret());
  assert.equal(verifySessionToken(token, generateSecret()), null);
});

test("rejects structurally invalid tokens", () => {
  const secret = generateSecret();
  assert.equal(verifySessionToken("", secret), null);
  assert.equal(verifySessionToken("no-dot", secret), null);
  assert.equal(verifySessionToken(".signature-only", secret), null);
});

test("admin slugs are URL-safe, 16+ characters, and unique", () => {
  const slug = newAdminSlug();
  assert.match(slug, /^[A-Za-z0-9_-]{16,}$/);
  assert.notEqual(newAdminSlug(), slug);
});

test("setup tokens are URL-safe, 24+ characters, and unique", () => {
  const token = newSetupToken();
  assert.match(token, /^[A-Za-z0-9_-]{24,}$/);
  assert.notEqual(newSetupToken(), token);
});

test("constant-time equality handles equal, unequal, and different-length inputs", () => {
  assert.equal(constantTimeEquals("secret-slug", "secret-slug"), true);
  assert.equal(constantTimeEquals("secret-slug", "secret-slug!"), false);
  assert.equal(constantTimeEquals("secret-slug", "another"), false);
  assert.equal(constantTimeEquals("", ""), true);
});

test("validates usernames", () => {
  assert.equal(validateUsername("admin"), null);
  assert.equal(validateUsername("jae-woong.gong_1"), null);
  assert.notEqual(validateUsername("ab"), null);
  assert.notEqual(validateUsername("-leading"), null);
  assert.notEqual(validateUsername("has space"), null);
  assert.notEqual(validateUsername("x".repeat(33)), null);
});

test("validates passwords", () => {
  assert.equal(validatePassword("long enough secret"), null);
  assert.notEqual(validatePassword("short"), null);
  assert.notEqual(validatePassword("x".repeat(257)), null);
});
