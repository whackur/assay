import { constantTimeEquals } from "@/lib/admin/auth";
import { defaultDataDir, getBootstrap } from "@/lib/admin/store";

// The admin area lives only under a secret per-deployment path segment,
// /panel-<slug> (Jenkins-style defense in depth: a capability URL on top of —
// never instead of — real session authentication). Every admin page and route
// handler resolves the incoming [panel] segment through this helper and
// renders the app's ordinary 404 when it does not match, so /admin, /setup,
// and wrong-slug guesses are indistinguishable from any other missing page.
//
// Operator recovery: the slug and (until setup completes) the one-time setup
// token are stored server-side in <data dir>/admin.json.

export const PANEL_PREFIX = "panel-";

export interface PanelContext {
  /** Public base path of the admin area, e.g. "/panel-abc123…" */
  basePath: string;
  configured: boolean;
  setupToken: string | null;
}

export async function resolvePanel(
  segment: string,
): Promise<PanelContext | null> {
  let bootstrap;
  try {
    bootstrap = await getBootstrap(defaultDataDir());
  } catch {
    // An unreadable store must fail closed as a plain 404, never a 500 that
    // singles the admin path out from the rest of the site.
    return null;
  }
  const expected = `${PANEL_PREFIX}${bootstrap.adminSlug}`;
  if (!constantTimeEquals(segment, expected)) return null;
  return {
    basePath: `/${expected}`,
    configured: bootstrap.configured,
    setupToken: bootstrap.setupToken,
  };
}
