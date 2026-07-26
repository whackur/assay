import { constantTimeEquals } from "@/lib/admin/auth";
import { defaultDataDir, getBootstrap } from "@/lib/admin/store";

// Admin area lives under a secret per-deployment path segment /panel-<slug>: a capability URL layered on top of (never instead of) session auth. Wrong-slug guesses render the ordinary 404 so /admin, /setup, and misses are indistinguishable from any other missing page. Slug and one-time setup token live in <data dir>/admin.json.

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
    // Unreadable store fails closed as a plain 404, never a 500 that singles the admin path out.
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
