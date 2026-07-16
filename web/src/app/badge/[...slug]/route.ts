import { fixtureApi } from "@/lib/api/client";
import { badgeSvg } from "@/lib/badge/badge";
import { isPublicResult } from "@/lib/state/result-state";

// Serves the README SVG badge for a completed evaluation (WEB-003). The badge
// is a pure function of the compiled result; this route performs no scoring.

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ slug: string[] }> },
) {
  const { slug } = await params;
  const id = slug.join("/");
  const record = await fixtureApi.getRecord(id);

  if (!record || record.state !== "complete" || !isPublicResult(record.evaluation)) {
    return new Response("Not found", { status: 404 });
  }

  return new Response(badgeSvg(record.evaluation), {
    status: 200,
    headers: {
      "content-type": "image/svg+xml; charset=utf-8",
      "cache-control": "public, max-age=300",
    },
  });
}
