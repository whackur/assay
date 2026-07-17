import { notFound } from "next/navigation";
import { NextRequest, NextResponse } from "next/server";
import { defaultDataDir, setEntryHidden } from "@/lib/admin/store";
import { requestSessionId } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";
import { RECORDS } from "@/lib/api/fixtures";

// Authenticated catalog visibility toggle. Hiding an entry removes it from the
// public catalog lists; the underlying evaluation record is untouched. Accepts
// a plain form post so the dashboard works without client JavaScript. The
// secret /panel-<slug> path gates the route; the session gates the action.

interface RouteContext {
  params: Promise<{ panel: string }>;
}

export async function POST(
  request: NextRequest,
  { params }: RouteContext,
): Promise<NextResponse> {
  const { panel } = await params;
  const context = await resolvePanel(panel);
  if (!context) notFound();

  const sessionId = await requestSessionId(request);
  if (!sessionId) {
    return NextResponse.json({ error: "Authentication required." }, { status: 401 });
  }

  const form = await request.formData();
  const entryId = form.get("entryId");
  const hidden = form.get("hidden");
  if (typeof entryId !== "string" || (hidden !== "true" && hidden !== "false")) {
    return NextResponse.json({ error: "Invalid visibility request." }, { status: 400 });
  }
  if (!(entryId in RECORDS)) {
    return NextResponse.json({ error: "Unknown catalog entry." }, { status: 404 });
  }

  await setEntryHidden(defaultDataDir(), entryId, hidden === "true");
  return NextResponse.redirect(new URL(context.basePath, request.url), 303);
}
