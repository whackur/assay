import type { Metadata } from "next";
import { notFound, redirect } from "next/navigation";
import { getAdminSessionId } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";
import { ssoEnabled } from "@/lib/admin/sso";
import { LoginForm } from "@/components/admin/LoginForm";

// Admin sign-in under the secret /panel-<slug> path. The form renders whether
// or not an administrator exists yet, so the page itself never reveals the
// deployment's setup state; an unconfigured deployment simply rejects the
// sign-in attempt. In SSO mode sign-in belongs to the identity provider, so
// this page is a plain 404 like any other missing route.

export const dynamic = "force-dynamic";

interface PageProps {
  params: Promise<{ panel: string }>;
}

export async function generateMetadata({
  params,
}: PageProps): Promise<Metadata> {
  const { panel } = await params;
  if (ssoEnabled() || !(await resolvePanel(panel))) notFound();
  return {
    title: "Admin sign-in — Assay",
    description: "Sign in to the Assay administration area.",
    robots: { index: false, follow: false },
  };
}

export default async function AdminLoginPage({ params }: PageProps) {
  const { panel } = await params;
  if (ssoEnabled()) notFound();
  const context = await resolvePanel(panel);
  if (!context) notFound();

  if (await getAdminSessionId()) {
    redirect(context.basePath);
  }

  return (
    <div className="auth-card">
      <h1>Admin sign-in</h1>
      <p className="lede">
        Administration covers deployment operations only. Public evaluations
        and scores are never edited from here.
      </p>
      <LoginForm basePath={context.basePath} />
    </div>
  );
}
