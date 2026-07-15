# Assay Git Adapter

`assay-git` collects bounded facts from immutable Git objects through the
installed Git CLI selected in ADR 0002. The adapter resolves a requested
revision to a full commit and root tree ID, enumerates tracked entries without
requiring UTF-8 paths, records modes and object kinds, hashes complete bounded
blobs with SHA-256, and reports bounded history and first-parent rename
availability. Repository SHA-1 or SHA-256 object format is detected once and
enforced across every parsed object ID. Shallow history remains explicitly
partial.

Commit timestamps are parsed and normalized to RFC 3339 UTC `Z`. Portable
local identity uses a versioned digest of object format, shallow state, and
sorted reachable root commits, never a host path. Identity derivation returns
the exact resolved commit that callers must collect.

The adapter does not inspect uncommitted working-tree state and does not
install, import, build, test, or execute repository code. It does not verify
that a project works, is safe, or has a particular quality. It disables lazy
fetch and network protocols, so a missing promised object remains unavailable
instead of being fetched.

Repository-local alternate object stores and symlinked object-store entries
are rejected before object access. This prevents a repository from redirecting
the collector to unrelated machine files. A linked Git worktree may use its
declared common directory, but that directory must have a direct, bounded
object store without alternate paths. Normal repositories, bare repositories,
and linked worktrees have distinct topology checks. A linked worktree is
accepted only when its administrative directory, common-directory relation,
and backlink all match the submitted worktree.

The threat model requires the deployment-selected Git executable and submitted
Git administrative metadata to remain unchanged during one collection. The
adapter never selects an executable from repository content. It validates the
repository topology and object store before object access and validates them
again before returning, reducing but not eliminating a concurrent filesystem
replacement race. Each Git invocation runs in a process group or Windows job;
the command deadline covers process exit and both output drains, and timeout
terminates the group.

Do not interpret an unavailable object as an empty or absent file. Do not
interpret a content hash as a quality, originality, or security signal. Limit
results are explicit partial evidence and must not be converted to zero.

Source bytes and raw diffs are not part of the snapshot contract. Errors expose
only a stable collection stage and failure category; they omit Git stderr,
repository paths, source content, and credential-bearing values.
