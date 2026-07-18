import { NextResponse } from "next/server";
import { createHash, createHmac } from "node:crypto";
import { internalApiBase } from "@/lib/api/hosted";
import {
  isHostedErrorResponse,
  isHostedProjectStatusResponse,
  isHostedSubmissionRequest,
  isHostedSubmissionResponse,
} from "@/lib/api/hosted.generated";

const BUCKET_HEADER = "x-assay-anonymous-bucket-id";

function anonymousBucket(request: Request): string {
  const trustedHeader = process.env.ASSAY_TRUSTED_CLIENT_IP_HEADER?.trim().toLowerCase();
  const key = process.env.ASSAY_ADMISSION_HASH_KEY?.trim();
  if (
    trustedHeader &&
    /^[a-z0-9-]{1,64}$/.test(trustedHeader) &&
    key &&
    key.length >= 32
  ) {
    const clientIdentity = request.headers.get(trustedHeader)?.trim();
    if (clientIdentity) {
      return createHmac("sha256", key)
        .update("assay-anonymous-client-v1\0")
        .update(clientIdentity.slice(0, 256))
        .digest("hex");
    }
  }
  return createHash("sha256").update("assay-shared-anonymous-v1").digest("hex");
}

export async function POST(request: Request) {
  const base = internalApiBase();
  if (!base) {
    return NextResponse.json({ error: { code: "api_unavailable" } }, { status: 503 });
  }
  const declaredLength = Number(request.headers.get("content-length") ?? "0");
  if (Number.isFinite(declaredLength) && declaredLength > 4_096) {
    return NextResponse.json({ error: { code: "request_too_large" } }, { status: 413 });
  }
  let body: unknown;
  try {
    const text = await request.text();
    if (text.length > 4_096) {
      return NextResponse.json({ error: { code: "request_too_large" } }, { status: 413 });
    }
    body = JSON.parse(text) as unknown;
  } catch {
    return NextResponse.json({ error: { code: "invalid_request" } }, { status: 400 });
  }
  if (!isHostedSubmissionRequest(body)) {
    return NextResponse.json({ error: { code: "invalid_request" } }, { status: 400 });
  }
  try {
    const response = await fetch(`${base}/internal/hosted/submissions`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        [BUCKET_HEADER]: anonymousBucket(request),
      },
      body: JSON.stringify(body),
      cache: "no-store",
      signal: AbortSignal.timeout(8_000),
    });
    const payload: unknown = await response.json();
    if (response.ok ? !isHostedSubmissionResponse(payload) : !isHostedErrorResponse(payload)) {
      return NextResponse.json({ error: { code: "invalid_upstream_contract" } }, { status: 502 });
    }
    const retryAfter = response.headers.get("retry-after");
    return NextResponse.json(payload, {
      status: response.status,
      headers: retryAfter ? { "retry-after": retryAfter } : undefined,
    });
  } catch {
    return NextResponse.json({ error: { code: "api_unavailable" } }, { status: 503 });
  }
}

export async function GET(request: Request) {
  const base = internalApiBase();
  if (!base) {
    return NextResponse.json({ error: { code: "api_unavailable" } }, { status: 503 });
  }
  const url = new URL(request.url);
  const owner = url.searchParams.get("owner")?.trim() ?? "";
  const repository = url.searchParams.get("repository")?.trim() ?? "";
  if (
    !/^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/.test(owner) ||
    !/^[A-Za-z0-9_.-]{1,100}$/.test(repository)
  ) {
    return NextResponse.json({ error: { code: "invalid_request" } }, { status: 400 });
  }
  try {
    const response = await fetch(
      `${base}/internal/hosted/projects/github/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}`,
      { cache: "no-store", signal: AbortSignal.timeout(4_000) },
    );
    const payload: unknown = await response.json();
    if (response.ok ? !isHostedProjectStatusResponse(payload) : !isHostedErrorResponse(payload)) {
      return NextResponse.json({ error: { code: "invalid_upstream_contract" } }, { status: 502 });
    }
    return NextResponse.json(payload, { status: response.status });
  } catch {
    return NextResponse.json({ error: { code: "api_unavailable" } }, { status: 503 });
  }
}
