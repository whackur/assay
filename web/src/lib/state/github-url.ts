import type { HostedSource } from "@/lib/contract/types";

// Submission-side validation for specification 12.1. The initial release accepts
// GitHub hosts only, preventing general URL fetching and SSRF via the field.
// This canonicalizes and validates input; it does not fetch anything.

const ALLOWED_HOSTS = new Set(["github.com", "www.github.com"]);
const SEGMENT = /^[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?$/;
const RESERVED_OWNERS = new Set([
  "settings",
  "marketplace",
  "sponsors",
  "features",
  "about",
  "login",
]);

export type GithubTargetResult =
  | { ok: true; source: HostedSource; canonicalUrl: string }
  | { ok: false; error: string };

function normalizeRepository(name: string): string {
  return name.endsWith(".git") ? name.slice(0, -4) : name;
}

function build(owner: string, repository: string): GithubTargetResult {
  const namespace = owner.toLowerCase();
  const repo = normalizeRepository(repository).toLowerCase();
  if (!SEGMENT.test(owner) || !SEGMENT.test(normalizeRepository(repository))) {
    return { ok: false, error: "Owner and repository may use letters, digits, dot, dash, and underscore." };
  }
  if (RESERVED_OWNERS.has(namespace)) {
    return { ok: false, error: "That path is a GitHub reserved route, not a repository." };
  }
  return {
    ok: true,
    source: { kind: "hosted", provider: "github", namespace, repository: repo },
    canonicalUrl: `https://github.com/${namespace}/${repo}`,
  };
}

export function parseGithubTarget(input: string): GithubTargetResult {
  const trimmed = input.trim();
  if (trimmed === "") {
    return { ok: false, error: "Enter a GitHub repository URL or owner/repository." };
  }

  const shorthand = trimmed.match(/^([^/\s]+)\/([^/\s]+)$/);
  if (shorthand && !trimmed.includes(":")) {
    return build(shorthand[1]!, shorthand[2]!);
  }

  let url: URL;
  try {
    url = new URL(trimmed.includes("://") ? trimmed : `https://${trimmed}`);
  } catch {
    return { ok: false, error: "Enter a valid GitHub repository URL." };
  }

  if (url.protocol !== "https:" && url.protocol !== "http:") {
    return { ok: false, error: "Only https GitHub URLs are accepted." };
  }
  if (!ALLOWED_HOSTS.has(url.hostname.toLowerCase())) {
    return { ok: false, error: "Only github.com repositories are accepted." };
  }

  const segments = url.pathname.split("/").filter(Boolean);
  if (segments.length < 2) {
    return { ok: false, error: "The URL must point to a repository, for example github.com/owner/repository." };
  }
  return build(segments[0]!, segments[1]!);
}
