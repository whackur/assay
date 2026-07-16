import assert from "node:assert/strict";
import { test } from "node:test";
import { parseGithubTarget } from "@/lib/state/github-url";

test("canonicalizes a full https URL and lowercases the identity", () => {
  const r = parseGithubTarget("https://github.com/Example-Org/Sample-Project");
  assert.ok(r.ok);
  assert.deepEqual(r.source, {
    kind: "hosted",
    provider: "github",
    namespace: "example-org",
    repository: "sample-project",
  });
  assert.equal(r.canonicalUrl, "https://github.com/example-org/sample-project");
});

test("accepts owner/repository shorthand", () => {
  const r = parseGithubTarget("example-org/sample-project");
  assert.ok(r.ok);
  assert.equal(r.source.repository, "sample-project");
});

test("strips a trailing .git and deep tree path", () => {
  const r = parseGithubTarget("github.com/example-org/sample-project.git/tree/main/src");
  assert.ok(r.ok);
  assert.equal(r.source.repository, "sample-project");
});

test("rejects non-github hosts to prevent SSRF", () => {
  const r = parseGithubTarget("https://gitlab.com/example-org/sample-project");
  assert.equal(r.ok, false);
});

test("rejects a host-only URL with no repository", () => {
  const r = parseGithubTarget("https://github.com/example-org");
  assert.equal(r.ok, false);
});

test("rejects empty input", () => {
  assert.equal(parseGithubTarget("   ").ok, false);
});
