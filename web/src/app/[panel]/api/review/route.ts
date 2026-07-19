import { notFound } from "next/navigation";
import { NextRequest, NextResponse } from "next/server";
import { requestAdminPrincipal } from "@/lib/admin/guard";
import { resolvePanel } from "@/lib/admin/panel";
import { approveReviewItem, getReviewQueue } from "@/lib/admin/review";

interface RouteContext { params: Promise<{ panel: string }> }
function sameOrigin(request: NextRequest): boolean {
  const origin = request.headers.get("origin");
  if (origin) return origin === request.nextUrl.origin;
  const referer = request.headers.get("referer");
  return !!referer && new URL(referer).origin === request.nextUrl.origin;
}

export async function GET(request: NextRequest, { params }: RouteContext) {
  const { panel } = await params; if (!(await resolvePanel(panel))) notFound();
  if (!(await requestAdminPrincipal(request))) return NextResponse.json({ error: "Authentication required." }, { status: 401 });
  const result = await getReviewQueue();
  return result.state === "available" ? NextResponse.json(result) : NextResponse.json({ error: "Review queue unavailable." }, { status: 503 });
}

export async function POST(request: NextRequest, { params }: RouteContext) {
  const { panel } = await params; if (!(await resolvePanel(panel))) notFound();
  if (!sameOrigin(request)) return NextResponse.json({ error: "Same-origin request required." }, { status: 403 });
  const principal = await requestAdminPrincipal(request);
  if (!principal) return NextResponse.json({ error: "Authentication required." }, { status: 401 });
  let body: unknown; try { body = await request.json(); } catch { return NextResponse.json({ error: "Invalid request." }, { status: 400 }); }
  const id = body && typeof body === "object" && "evaluation_snapshot_id" in body ? (body as { evaluation_snapshot_id: unknown }).evaluation_snapshot_id : null;
  if (typeof id !== "string" || id.length < 1 || id.length > 128) return NextResponse.json({ error: "Evaluation snapshot is required." }, { status: 400 });
  const status = await approveReviewItem(id, principal);
  if (status === 204) return NextResponse.json({ ok: true });
  if (status === 400 || status === 404) return NextResponse.json({ error: "Evaluation cannot be published." }, { status });
  return NextResponse.json({ error: "Publishing service unavailable." }, { status: 503 });
}
