import { notFound } from "next/navigation";
import { NextRequest, NextResponse } from "next/server";
import { hashPassword, verifyPassword } from "@/lib/admin/auth";
import { createSession, defaultDataDir, getAdmin } from "@/lib/admin/store";
import { setSessionCookie } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";
import { ssoEnabled } from "@/lib/admin/sso";

// Admin sign-in, reachable only under the secret /panel-<slug> path; a wrong
// slug is a plain 404 before any credential handling happens. In SSO mode
// local credentials do not exist, so the endpoint is a plain 404 as well.

// A fixed dummy hash keeps the work factor constant when the username does not
// match, so login timing does not reveal whether an account name exists.
const dummyHashPromise = hashPassword("assay-dummy-password-for-timing");

interface RouteContext {
  params: Promise<{ panel: string }>;
}

export async function POST(
  request: NextRequest,
  { params }: RouteContext,
): Promise<NextResponse> {
  const { panel } = await params;
  if (ssoEnabled()) notFound();
  if (!(await resolvePanel(panel))) notFound();

  const dir = defaultDataDir();
  const admin = await getAdmin(dir);
  if (!admin) {
    // Only holders of the secret path get this far, so pointing the operator
    // back at the server-console setup URL leaks nothing to outsiders.
    return NextResponse.json(
      {
        error:
          "No administrator is configured yet. Use the one-time setup URL from the server logs.",
        setupRequired: true,
      },
      { status: 409 },
    );
  }

  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return NextResponse.json({ error: "Invalid request body." }, { status: 400 });
  }
  const { username, password } = (body ?? {}) as {
    username?: unknown;
    password?: unknown;
  };
  if (typeof username !== "string" || typeof password !== "string") {
    return NextResponse.json(
      { error: "Username and password are required." },
      { status: 400 },
    );
  }

  const usernameMatches = username.trim() === admin.username;
  const hashToCheck = usernameMatches ? admin.passwordHash : await dummyHashPromise;
  const passwordMatches = await verifyPassword(password, hashToCheck);

  if (!usernameMatches || !passwordMatches) {
    return NextResponse.json(
      { error: "Invalid username or password." },
      { status: 401 },
    );
  }

  const session = await createSession(dir);
  const response = NextResponse.json({ ok: true });
  setSessionCookie(response, request, session.cookieValue);
  return response;
}
