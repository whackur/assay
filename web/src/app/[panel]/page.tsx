import type { Metadata } from "next";
import { notFound, redirect } from "next/navigation";
import { getAdminSessionId } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";
import {
  defaultDataDir,
  getAdmin,
  hiddenEntryIds,
  loadOrInitState,
} from "@/lib/admin/store";
import { ssoEnabled, ssoLoginRedirect } from "@/lib/admin/sso";
import { publicCatalogEntries } from "@/lib/catalog/fixtures";
import { scoreSummary } from "@/lib/catalog/catalog";
import { ReviewQueue } from "@/components/admin/ReviewQueue";

// Admin dashboard, reachable only under the secret /panel-<slug> path. A
// wrong slug renders the app's ordinary 404 before anything admin-flavored
// (including metadata) is produced.

export const dynamic = "force-dynamic";

interface PageProps {
  params: Promise<{ panel: string }>;
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { panel } = await params;
  if (!(await resolvePanel(panel))) notFound();
  return {
    title: "Administration — Assay",
    description: "Deployment administration for this Assay instance.",
    robots: { index: false, follow: false },
  };
}

export default async function AdminPage({ params }: PageProps) {
  const { panel } = await params;
  const context = await resolvePanel(panel);
  if (!context) notFound();

  const sso = ssoEnabled();
  const sessionId = await getAdminSessionId();
  if (!sessionId) {
    if (sso) {
      // SSO mode has no local sign-in page. Hand off to the IdP when a login
      // URL is configured; otherwise the page is indistinguishable from a
      // wrong slug.
      const target = ssoLoginRedirect(context.basePath);
      if (target) redirect(target);
      notFound();
    }
    redirect(`${context.basePath}/login`);
  }

  const dir = defaultDataDir();
  const admin = await getAdmin(dir);
  const state = await loadOrInitState(dir);
  const hidden = new Set(await hiddenEntryIds(dir));
  const entries = publicCatalogEntries();
  const now = new Date().toISOString();
  const liveSessions = state.sessions.filter((s) => s.expiresAt > now).length;

  return (
    <div>
      <div className="admin-bar">
        <h1>Administration</h1>
        <span className="chip">deployment operations</span>
        {!sso && (
          <form method="post" action={`${context.basePath}/api/logout`}>
            <button type="submit" className="quiet">
              Sign out
            </button>
          </form>
        )}
      </div>

      <section className="admin-section" aria-labelledby="admin-deployment">
        <h2 id="admin-deployment">Deployment</h2>
        <dl className="meta">
          <dt>Administrator</dt>
          <dd>
            {sso ? (
              <>
                External identity provider{" "}
                <span className="muted">(SSO mode; local accounts disabled)</span>
              </>
            ) : (
              <>
                {admin?.username}{" "}
                <span className="muted">(created {admin?.createdAt.slice(0, 10)})</span>
              </>
            )}
          </dd>
          <dt>Setup state</dt>
          <dd className="status-ok">
            {sso
              ? "SSO — admin access is granted by the identity provider's role claim"
              : "Configured — the one-time setup token has been consumed"}
          </dd>
          <dt>Admin path</dt>
          <dd>
            <code>{context.basePath}</code>{" "}
            <span className="muted">
              (secret per-deployment path; recover it from the data directory)
            </span>
          </dd>
          {!sso && (
            <>
              <dt>Live sessions</dt>
              <dd>{liveSessions}</dd>
            </>
          )}
          <dt>Data directory</dt>
          <dd>
            <code>{dir}</code>
          </dd>
          <dt>Evaluation engine</dt>
          <dd>Fixture preview — no live Rust engine is attached to this deployment</dd>
        </dl>
      </section>

      <section className="admin-section" aria-labelledby="admin-catalog">
        <h2 id="admin-catalog">Catalog visibility</h2>
        <p className="lede">
          Hiding an entry removes it from the public catalog lists. The
          underlying evaluation record and its score are never edited.
        </p>
        <table className="admin-table">
          <thead>
            <tr>
              <th scope="col">Project</th>
              <th scope="col">Assay Score</th>
              <th scope="col">State</th>
              <th scope="col">
                <span className="visually-hidden">Action</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => {
              const summary = scoreSummary(entry.score);
              const isHidden = hidden.has(entry.id);
              return (
                <tr key={entry.id} className={isHidden ? "row-hidden" : undefined}>
                  <td>
                    <code>{entry.id}</code>
                  </td>
                  <td>{summary.released ? summary.valueText : summary.statusLabel}</td>
                  <td>{isHidden ? "Hidden from catalog" : "Listed"}</td>
                  <td>
                    <form method="post" action={`${context.basePath}/api/catalog`}>
                      <input type="hidden" name="entryId" value={entry.id} />
                      <input type="hidden" name="hidden" value={isHidden ? "false" : "true"} />
                      <button type="submit" className="quiet">
                        {isHidden ? "Show" : "Hide"}
                      </button>
                    </form>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </section>

      <section className="admin-section" aria-labelledby="admin-pending">
        <h2 id="admin-pending">Evaluation operations</h2>
        <ReviewQueue panel={context.basePath} />
      </section>
    </div>
  );
}
