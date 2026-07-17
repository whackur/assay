import { notFound } from "next/navigation";
import { NextRequest, NextResponse } from "next/server";
import { defaultDataDir, removeSession } from "@/lib/admin/store";
import { clearSessionCookie, requestSessionId } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";

// Logout deletes the server-side session record and clears the cookie. Plain
// HTML form post from the dashboard, so it works without client JavaScript.
// Lives under the secret /panel-<slug> path like every other admin surface.

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
  if (sessionId) {
    await removeSession(defaultDataDir(), sessionId);
  }
  const response = NextResponse.redirect(
    new URL(`${context.basePath}/login`, request.url),
    303,
  );
  clearSessionCookie(response, request);
  return response;
}
