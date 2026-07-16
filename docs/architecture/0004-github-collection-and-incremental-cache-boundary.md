# ADR 0004: Pin the GitHub Collection and Incremental Cache Boundary

- Status: Accepted
- Date: 2026-07-16

## Context

Public web analysis and local private analysis both need read-only GitHub
collection before any score exists. The collection surface parses
attacker-influenced input at three points: the submitted repository field, the
selected revision, and the streamed Git tree. It also feeds an incremental
cache whose keys must never let one immutable evaluation be served for a
different one.

The specification requires GitHub-host-only submission that prevents general
URL fetching and SSRF, immutable commit resolution, an explicit rate-limit
posture, and an evaluation identity built from provider repository ID,
immutable commit, evidence extractor version, evaluation and scoring version,
rubric version, and canonical evaluator profile. Account identity scopes
visibility and refresh admission but is not part of the content key. This crate
implements the deterministic seams for that contract; persistent HTTP and cache
adapters live outside it.

## Decision

### Fixed-origin, GET-only transport

The `GitHubHttp` seam expresses only a path relative to a fixed
`https://api.github.com` origin. A request cannot carry a method other than
GET and cannot carry an authorization value, so public collection cannot be
turned into a write or an authenticated call by malformed input. The injected
transport MUST pin the origin and MUST NOT follow redirects across it. Any
non-200 status is a fail-closed collection error; a 3xx response is never
followed and its `Location` is never read, so a moved repository cannot
redirect collection to another host or leak a target path.

### Bounded, non-leaking request and response handling

The submitted repository is canonicalized to a lowercase `owner/name` before
any request; percent, query, fragment, userinfo, port, and non-GitHub hosts are
rejected without echoing the input. A selected ref is length-bounded, rejects
control bytes, and is percent-encoded as a single path segment so a ref cannot
inject additional path structure. Every response is read through a hard byte
bound in addition to an early `Content-Length` check, so a truncated or
oversized body fails closed with a stable `response_limit` error rather than a
fabricated result. Rate-limit headers are parsed into an explicit
`Available`, `Exhausted`, `SecondaryLimited`, or `Unknown` state; missing,
non-numeric, negative, or `Retry-After` date values collapse to `Unknown` or an
absent delay and are never read as unlimited capacity. A successful resolution
still reports an exhausted budget so callers can back off. Errors carry only a
stable stage and machine code; response bodies, ref values, paths, tokens, and
host paths never enter an error or its `Display`.

### Immutable resolution and provider-authoritative canonical

Resolution reads repository metadata, refuses a private repository, and peels
the selected ref to a full immutable object identifier through the commits
endpoint. Only a full, non-null, lowercase object identifier is accepted as the
revision. A duplicated provider field such as a second `private` is rejected by
strict struct decoding, so a repeated key cannot reopen a private repository;
unrelated provider extension fields are ignored for forward compatibility.

### Bounded streaming tree collection

The recursive tree response is streamed entry by entry. The full payload and
each source body are never retained; only bounded counters, a bounded project
boundary set, and the entries handed one at a time to the downstream sink are
kept. Provider truncation, an entry-count bound, an over-long or unsafe path,
and a boundary-count bound each produce an explicit `partial` reason rather than
a silent zero or a hard failure. An unsafe relative path, including an absolute
or parent-traversal path, is dropped as `path_limit` partial and never reaches
the sink. An invalid blob object identifier fails closed as an invalid provider
response rather than being dropped. The response byte bound remains a hard
fail-closed error because a truncated JSON structure cannot be reported as an
honest partial.

### Fully versioned, account-independent cache keys

The evaluation key is a domain-separated, length-prefixed SHA-256 over the
provider tag, provider repository ID, immutable revision, evidence version,
evaluation version, rubric version, and evaluator profile. Every one of those
dimensions changes the digest, and the material contains no account identity.
The blob-analysis key is a separately domain-separated digest over the blob
object identifier, analyzer version, and rule-set hash, so a blob analysis is
reused only when the object and both versioned policies match. Length
prefixing prevents a component-boundary shift from producing a colliding key,
and the distinct domain tags keep blob and evaluation keys from colliding.
Cache lookups return `Hit`, `Miss`, `InFlight` where meaningful, or
`Unavailable`; an unavailable lookup is preserved and never treated as a miss.

## Consequences

- SSRF and write escalation are excluded at the type level: the seam cannot
  express a non-GitHub origin, a non-GET method, an authorization value, or a
  followed redirect.
- Rate budget, truncation, and partial collection are explicit states, never a
  misleading zero, satisfying the unavailable-not-zero policy.
- Persistent HTTP and cache adapters remain outside the crate and inherit these
  contracts through the deterministic seams and their tests.
- Quota accounting, cooldown windows, live network transport, tokens, private
  repositories, and score compilation are deliberately out of scope and belong
  to later milestones.

## Alternatives considered

- A general URL or method seam was rejected because it would allow SSRF and
  non-read requests through malformed input.
- Following provider redirects inside the crate was rejected because it could
  reach another origin and leak a target path.
- Treating a missing or unparsable rate-limit header as unlimited was rejected
  because it would hide an exhausted budget.
- Folding account identity into the evaluation key was rejected because it
  would fragment an otherwise shared immutable result and contradicts the
  specification's content-identity definition.
- Buffering the whole tree before analysis was rejected because it would retain
  the full payload and source bodies and defeat large-repository streaming.
