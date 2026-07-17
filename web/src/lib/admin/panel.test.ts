import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { PANEL_PREFIX, resolvePanel } from "@/lib/admin/panel";
import { getBootstrap } from "@/lib/admin/store";

// resolvePanel reads the store through defaultDataDir(), so point the data
// directory at a throwaway location for this test process (lazily — the test
// runner transpiles to CJS, which forbids top-level await).
let dataDirPromise: Promise<string> | null = null;

function testDataDir(): Promise<string> {
  dataDirPromise ??= mkdtemp(path.join(tmpdir(), "assay-admin-panel-")).then(
    (dir) => {
      process.env.ASSAY_DATA_DIR = dir;
      return dir;
    },
  );
  return dataDirPromise;
}

test("resolves the deployment's own slug-prefixed segment", async () => {
  const bootstrap = await getBootstrap(await testDataDir());
  const context = await resolvePanel(`${PANEL_PREFIX}${bootstrap.adminSlug}`);
  assert.ok(context);
  assert.equal(context!.basePath, `/${PANEL_PREFIX}${bootstrap.adminSlug}`);
  assert.equal(context!.configured, false);
  assert.equal(context!.setupToken, bootstrap.setupToken);
});

test("rejects every other segment so it falls through to the app's 404", async () => {
  const bootstrap = await getBootstrap(await testDataDir());
  // The guessable names an attacker would probe first.
  assert.equal(await resolvePanel("admin"), null);
  assert.equal(await resolvePanel("setup"), null);
  assert.equal(await resolvePanel(PANEL_PREFIX.replace(/-$/, "")), null);
  // Right prefix, wrong slug.
  assert.equal(await resolvePanel(`${PANEL_PREFIX}0000000000000000`), null);
  // The bare slug without the prefix.
  assert.equal(await resolvePanel(bootstrap.adminSlug), null);
  // A prefix of the real segment.
  assert.equal(
    await resolvePanel(`${PANEL_PREFIX}${bootstrap.adminSlug}`.slice(0, -1)),
    null,
  );
});
