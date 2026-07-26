import path from "node:path";
import { defaultDataDir, getBootstrap } from "@/lib/admin/store";
import { PANEL_PREFIX } from "@/lib/admin/panel";
import { ssoEnabled } from "@/lib/admin/sso";

// First-boot console banner (Jenkins initialAdminPassword pattern): while no admin exists, every server start prints the one-time setup URL (slug + token) to stdout, where only the operator can read it. Nothing on the public site links to or hints at the admin area.

export async function printFirstRunBannerIfNeeded(): Promise<void> {
  const dir = defaultDataDir();
  // SSO mode has no local setup flow (setup URL would 404), but the operator still needs the secret panel path from the store.
  if (ssoEnabled()) {
    try {
      const bootstrap = await getBootstrap(dir);
      console.log(
        `[assay] SSO mode: admin area is at /${PANEL_PREFIX}${bootstrap.adminSlug} (local setup/login disabled).`,
      );
    } catch (error) {
      console.error(`[assay] Could not read the admin store in ${dir}:`, error);
    }
    return;
  }
  let bootstrap;
  try {
    bootstrap = await getBootstrap(dir);
  } catch (error) {
    console.error(`[assay] Could not read the admin store in ${dir}:`, error);
    return;
  }
  if (bootstrap.configured || !bootstrap.setupToken) return;

  const port = process.env.PORT ?? "3000";
  const basePath = `/${PANEL_PREFIX}${bootstrap.adminSlug}`;
  const setupUrl = `http://localhost:${port}${basePath}/setup?token=${bootstrap.setupToken}`;
  const storeFile = path.join(dir, "admin.json");

  console.log(
    [
      "",
      "*************************************************************",
      "*",
      "*  Assay first-run setup",
      "*",
      "*  No administrator is configured for this deployment.",
      "*  Create the admin account at:",
      "*",
      `*    ${setupUrl}`,
      "*",
      "*  The one-time setup token in that URL is required and is",
      "*  invalidated as soon as setup succeeds. Afterwards the",
      `*  admin area stays at ${basePath}`,
      "*",
      "*  Lost this URL? The panel slug and setup token are stored",
      "*  server-side in:",
      `*    ${storeFile}`,
      "*",
      "*************************************************************",
      "",
    ].join("\n"),
  );
}
