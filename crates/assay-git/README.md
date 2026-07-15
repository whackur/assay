# Assay Git Adapter

`assay-git` collects bounded facts from immutable Git objects through the
installed Git CLI selected in ADR 0002. The adapter resolves a requested
revision to a full commit and root tree ID, enumerates tracked entries without
requiring UTF-8 paths, records modes and object kinds, hashes complete bounded
blobs with SHA-256, and reports bounded history and first-parent rename
availability.

The adapter does not inspect uncommitted working-tree state and does not
install, import, build, test, or execute repository code. It does not verify
that a project works, is safe, or has a particular quality. It disables lazy
fetch and network protocols, so a missing promised object remains unavailable
instead of being fetched.

Repository-local alternate object stores and symlinked object-store entries
are rejected before object access. This prevents a repository from redirecting
the collector to unrelated machine files. A linked Git worktree may use its
declared common directory, but that directory must have a direct, bounded
object store without alternate paths.

Do not interpret an unavailable object as an empty or absent file. Do not
interpret a content hash as a quality, originality, or security signal. Limit
results are explicit partial evidence and must not be converted to zero.

Source bytes and raw diffs are not part of the snapshot contract. Errors expose
only a stable collection stage and failure category; they omit Git stderr,
repository paths, source content, and credential-bearing values.
