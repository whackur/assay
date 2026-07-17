import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { hashPassword, verifySessionToken } from "@/lib/admin/auth";
import {
  claimAdmin,
  createSession,
  findLiveSession,
  getBootstrap,
  getSessionSecret,
  hiddenEntryIds,
  isConfigured,
  loadOrInitState,
  removeSession,
  setEntryHidden,
} from "@/lib/admin/store";

function freshDir(): Promise<string> {
  return mkdtemp(path.join(tmpdir(), "assay-admin-store-"));
}

test("initializes an empty store with secret, slug, and setup token", async () => {
  const dir = await freshDir();
  const state = await loadOrInitState(dir);
  assert.equal(state.admin, null);
  assert.ok(state.sessionSecret.length > 20);
  assert.match(state.adminSlug, /^[A-Za-z0-9_-]{16,}$/);
  assert.ok(state.setupToken);
  assert.match(state.setupToken!, /^[A-Za-z0-9_-]{24,}$/);
  assert.equal(await isConfigured(dir), false);

  const onDisk = JSON.parse(await readFile(path.join(dir, "admin.json"), "utf8"));
  assert.equal(onDisk.version, 1);
});

test("each deployment gets its own slug and token", async () => {
  const a = await loadOrInitState(await freshDir());
  const b = await loadOrInitState(await freshDir());
  assert.notEqual(a.adminSlug, b.adminSlug);
  assert.notEqual(a.setupToken, b.setupToken);
});

test("claims the admin exactly once with the setup token", async () => {
  const dir = await freshDir();
  const hash = await hashPassword("a long enough password");
  const { setupToken } = await getBootstrap(dir);
  assert.ok(setupToken);

  const first = await claimAdmin(dir, setupToken!, "operator", hash);
  assert.equal(first.ok, true);
  assert.equal(await isConfigured(dir), true);

  const second = await claimAdmin(dir, setupToken!, "intruder", hash);
  assert.equal(second.ok, false);
  if (!second.ok) assert.equal(second.error, "already_configured");
});

test("rejects setup without the valid token", async () => {
  const dir = await freshDir();
  const hash = await hashPassword("a long enough password");
  await loadOrInitState(dir);

  const result = await claimAdmin(dir, "wrong-token", "intruder", hash);
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.error, "invalid_token");
  assert.equal(await isConfigured(dir), false);
});

test("consumes the setup token on successful setup", async () => {
  const dir = await freshDir();
  const hash = await hashPassword("a long enough password");
  const { setupToken } = await getBootstrap(dir);

  const claimed = await claimAdmin(dir, setupToken!, "operator", hash);
  assert.equal(claimed.ok, true);

  const after = await getBootstrap(dir);
  assert.equal(after.configured, true);
  assert.equal(after.setupToken, null);
});

test("getBootstrap exposes slug and token for the console banner", async () => {
  const dir = await freshDir();
  const state = await loadOrInitState(dir);
  const bootstrap = await getBootstrap(dir);
  assert.equal(bootstrap.configured, false);
  assert.equal(bootstrap.adminSlug, state.adminSlug);
  assert.equal(bootstrap.setupToken, state.setupToken);
});

test("migrates a pre-hardening store in place, keeping the admin", async () => {
  const dir = await freshDir();
  const legacy = {
    version: 1,
    admin: {
      username: "operator",
      passwordHash: "scrypt$16384$8$1$x$y",
      createdAt: "2026-01-01T00:00:00.000Z",
    },
    sessionSecret: "legacy-secret-legacy-secret",
    sessions: [],
    hiddenEntryIds: ["acme/example"],
  };
  await writeFile(
    path.join(dir, "admin.json"),
    JSON.stringify(legacy),
    "utf8",
  );

  const state = await loadOrInitState(dir);
  assert.equal(state.admin?.username, "operator");
  assert.match(state.adminSlug, /^[A-Za-z0-9_-]{16,}$/);
  // Configured deployments never get a (re)usable setup token.
  assert.equal(state.setupToken, null);
  assert.deepEqual(state.hiddenEntryIds, ["acme/example"]);

  const onDisk = JSON.parse(await readFile(path.join(dir, "admin.json"), "utf8"));
  assert.equal(onDisk.adminSlug, state.adminSlug);
  assert.equal(onDisk.setupToken, null);
});

test("migrates an unconfigured pre-hardening store with a fresh token", async () => {
  const dir = await freshDir();
  const legacy = {
    version: 1,
    admin: null,
    sessionSecret: "legacy-secret-legacy-secret",
    sessions: [],
    hiddenEntryIds: [],
  };
  await writeFile(
    path.join(dir, "admin.json"),
    JSON.stringify(legacy),
    "utf8",
  );

  const state = await loadOrInitState(dir);
  assert.match(state.adminSlug, /^[A-Za-z0-9_-]{16,}$/);
  assert.ok(state.setupToken);
});

test("issues a session whose cookie value verifies against the stored secret", async () => {
  const dir = await freshDir();
  const issued = await createSession(dir);
  const secret = await getSessionSecret(dir);
  assert.ok(secret);
  assert.equal(verifySessionToken(issued.cookieValue, secret!), issued.record.id);

  const found = await findLiveSession(dir, issued.record.id);
  assert.ok(found);
  assert.equal(found!.id, issued.record.id);
});

test("expired sessions are not live and are pruned on the next issue", async () => {
  const dir = await freshDir();
  const past = "2020-01-01T00:00:00.000Z";
  const stale = await createSession(dir, past);

  const now = new Date().toISOString();
  assert.equal(await findLiveSession(dir, stale.record.id, now), null);

  await createSession(dir, now);
  const state = await loadOrInitState(dir);
  assert.equal(state.sessions.some((s) => s.id === stale.record.id), false);
});

test("removes a session on logout", async () => {
  const dir = await freshDir();
  const issued = await createSession(dir);
  await removeSession(dir, issued.record.id);
  assert.equal(await findLiveSession(dir, issued.record.id), null);
});

test("toggles catalog entry visibility idempotently", async () => {
  const dir = await freshDir();
  assert.deepEqual(await hiddenEntryIds(dir), []);

  await setEntryHidden(dir, "acme/example", true);
  await setEntryHidden(dir, "acme/example", true);
  assert.deepEqual(await hiddenEntryIds(dir), ["acme/example"]);

  await setEntryHidden(dir, "acme/example", false);
  assert.deepEqual(await hiddenEntryIds(dir), []);
});

test("a missing store never breaks public reads", async () => {
  const dir = path.join(await freshDir(), "does-not-exist-yet");
  assert.deepEqual(await hiddenEntryIds(dir), []);
});
