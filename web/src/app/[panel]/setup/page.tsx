import type { Metadata } from "next";
import { notFound, redirect } from "next/navigation";
import { constantTimeEquals } from "@/lib/admin/auth";
import { resolvePanel, type PanelContext } from "@/lib/admin/panel";
import { SetupForm } from "@/components/admin/SetupForm";

// First-run setup, gated twice: the secret /panel-<slug> path AND the
// one-time setup token printed to the server console at boot. Without both,
// the page is the app's ordinary 404. Once an admin exists the token is
// consumed and this page permanently redirects to sign-in.

export const dynamic = "force-dynamic";

interface PageProps {
  params: Promise<{ panel: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

function validSetupToken(
  context: PanelContext,
  token: string | string[] | undefined,
): token is string {
  return (
    typeof token === "string" &&
    context.setupToken !== null &&
    constantTimeEquals(token, context.setupToken)
  );
}

export async function generateMetadata({
  params,
  searchParams,
}: PageProps): Promise<Metadata> {
  const { panel } = await params;
  const context = await resolvePanel(panel);
  if (!context) notFound();
  if (!context.configured && !validSetupToken(context, (await searchParams).token)) {
    notFound();
  }
  return {
    title: "First-run setup — Assay",
    description: "Create the administrator account for this Assay deployment.",
    robots: { index: false, follow: false },
  };
}

export default async function SetupPage({ params, searchParams }: PageProps) {
  const { panel } = await params;
  const context = await resolvePanel(panel);
  if (!context) notFound();

  if (context.configured) {
    // Only reachable by someone who already holds the secret path; send the
    // operator's stale bookmark to sign-in instead of a dead end.
    redirect(`${context.basePath}/login`);
  }

  const token = (await searchParams).token;
  if (!validSetupToken(context, token)) notFound();

  return (
    <div className="auth-card">
      <h1>First-run setup</h1>
      <p className="lede">
        This deployment has no administrator yet. Create the admin account to
        finish setup; the one-time setup token is consumed when it succeeds.
      </p>
      <SetupForm basePath={context.basePath} token={token} />
    </div>
  );
}
