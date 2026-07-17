import path from "node:path";
import { defaultDataDir, getBootstrap } from "@/lib/admin/store";
import { PANEL_PREFIX } from "@/lib/admin/panel";

// First-boot console banner (Jenkins initialAdminPassword pattern). While no
// administrator exists, every server start prints the one-time setup URL —
// secret panel slug plus setup token — to stdout, where only the operator
// (terminal or `docker logs`) can read it. Nothing on the public site ever
// links to or hints at the admin area.

export async function printFirstRunBannerIfNeeded(): Promise<void> {
  const dir = defaultDataDir();
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
