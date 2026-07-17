import { notFound } from "next/navigation";
import { NextRequest, NextResponse } from "next/server";
import {
  constantTimeEquals,
  hashPassword,
  validatePassword,
  validateUsername,
} from "@/lib/admin/auth";
import { claimAdmin, createSession, defaultDataDir } from "@/lib/admin/store";
import { setSessionCookie } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";

// First-run admin creation, gated twice: the secret /panel-<slug> path AND the
// one-time setup token from the server console banner. A wrong slug or a
// missing/invalid token is a plain 404 (notFound() is supported in Next 16
// route handlers and yields the same bodyless 404), so probing this endpoint
// reveals nothing. The token is consumed atomically inside claimAdmin the
// moment setup succeeds.

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

  let body: unknown;
  try {
    body = await request.json();
  } catch {
    body = null;
  }
  const { token, username, password } = (body ?? {}) as {
    token?: unknown;
    username?: unknown;
    password?: unknown;
  };

  // Token gate before anything else: requests without the valid one-time
  // token get the same 404 as a wrong slug — no existence oracle.
  if (
    typeof token !== "string" ||
    context.setupToken === null ||
    !constantTimeEquals(token, context.setupToken)
  ) {
    notFound();
  }

  if (typeof username !== "string" || typeof password !== "string") {
    return NextResponse.json(
      { error: "Username and password are required." },
      { status: 400 },
    );
  }
  const usernameError = validateUsername(username.trim());
  if (usernameError) {
    return NextResponse.json({ error: usernameError }, { status: 400 });
  }
  const passwordError = validatePassword(password);
  if (passwordError) {
    return NextResponse.json({ error: passwordError }, { status: 400 });
  }

  // claimAdmin re-checks the token inside a serialized mutation, so a
  // concurrent claim (or an already-consumed token) still fails closed here.
  const claimed = await claimAdmin(
    defaultDataDir(),
    token,
    username.trim(),
    await hashPassword(password),
  );
  if (!claimed.ok) notFound();

  const session = await createSession(defaultDataDir());
  const response = NextResponse.json({ ok: true }, { status: 201 });
  setSessionCookie(response, request, session.cookieValue);
  return response;
}
