# ADR 0009: Provider-Agnostic Identity Boundary and Signature-Verification Port

- Status: Accepted
- Date: 2026-07-16

## Context

Assay supports optional member accounts through standards-compatible OpenID
Connect without depending on any particular website, identity implementation,
user database, or deployment. The first deployment connects hakhub.net as the
single upstream issuer, validates an Assay-specific audience, issues its own
opaque session, and maps a trusted upstream `admin` role to the Assay
Administrator role only through explicit deployment policy.

The specification requires the core codebase to avoid an upstream user table,
role enum, hard-coded issuer, JWKS URL, claim name, or shared browser cookie,
and to key identities by the validated `(issuer, subject)` pair rather than
email. It requires issuer, signature, audience, time, nonce, state, PKCE, and
exact-redirect validation to fail closed, an opaque rotated session that never
exposes upstream tokens, local entitlement authorization, and auditable
privileged mappings that carry no secret values.

The rest of the workspace has no HTTP client, async runtime, or cryptographic
signature stack, and the existing crates (ADR 0004, ADR 0006) keep deterministic
logic separate from injected I/O adapters. Identity validation is security
critical and must be exhaustively testable without network access, real JWKS
material, or wall-clock time.

## Decision

### A dedicated, provider-agnostic `assay-identity` crate

Normalized external identities, OIDC validation, sessions, roles, and
entitlements live in `crates/assay-identity`. It depends only on `serde` and
`sha2`. The issuer, client id, audience, redirect allowlist, allowed
algorithms, and any trusted admin claim are configuration values on
`OidcDeploymentConfig`; no provider domain, private claim convention, or role
enum is compiled in. hakhub.net appears only in the `configs/` example and in
tests, never in the crate.

### Signature and JWKS crypto behind an injected port

A `SignatureVerifier` port verifies one compact token against the issuer JWKS
and returns the algorithm it proved plus the recovered `VerifiedClaims`; it
enforces no policy. All policy — allowed algorithm, issuer, audience and
authorized party, expiration, not-before, issued-at with leeway, nonce, and
subject presence — stays in the pure `TokenValidator`, which fails closed with a
redacted `ValidationError`. This mirrors ADR 0006: no live HTTP client, JWKS
cache, or asymmetric-crypto dependency enters the workspace for this card, and a
deterministic fake verifier drives the full contract-test matrix. Wiring a real
verifier (JWKS discovery, key rotation, RS/ES/PS signature checks) is deferred to
the API deployment card that implements the port. The `SigningAlgorithm` enum
omits `none` and symmetric HMAC, so a downgraded or unsigned token is
unrepresentable rather than merely rejected.

The clock and entropy are also injected (`Clock`, `EntropySource`) so time and
secret generation are deterministic in tests. `sha2` is a direct dependency only
for the standard PKCE `S256` challenge, a deterministic transform; base64url is
implemented in-crate to avoid an extra dependency.

### Durable key, opaque session, and single-issuer profile

The account key is `AccountKey(IssuerUrl, Subject)`; email is a non-keying
claim, so an email change neither creates a new account nor merges distinct
subjects. The authorization-code flow issues and single-uses `state`, `nonce`,
and a PKCE verifier through an `AuthorizationStore`; a reused or unknown state
and a non-allowlisted or mismatched redirect fail closed. The `Session` is an
opaque, rotatable, revocable, expiring record that stores the account key and a
redacted cookie secret and never holds an upstream token. The single-issuer
profile is a configuration flag that disables independent registration; a token
minted only for the upstream application's own audience is rejected.

### Local authorization and auditable admin mapping

Authorization is local: `EntitlementPolicy` maps `LocalRole` to `Entitlement`
bundles, and handlers authorize a specific action. External roles never elevate
on their own. `AdministratorMappingPolicy` grants Administrator only when a
verified claim matches an explicitly configured `TrustedAdminClaim`, comparing
values as opaque strings with no imported provider enum. Every privileged
mapping emits an `AuditEvent` recording the action, account key, matched claim
name, and policy version, and no secret value.

### Secrets never leak through types

`UpstreamIdToken`, `State`, `Nonce`, `PkceVerifier`, and `SessionSecret` wrap
their material, implement a redacting `Debug`, and derive neither `Display` nor
`Serialize`; each exposes its value only through a named `reveal`. Validation
and value errors carry a stable kind and reason but never the rejected value.
Redaction is proven by tests over `Debug` output and audit serialization.

## Consequences

- Identity validation, session lifecycle, entitlement authorization, and admin
  mapping are fully unit- and contract-tested with deterministic fakes and no
  network, filesystem, process, or wall-clock dependency.
- A deployment implements `SignatureVerifier`, `Clock`, and `EntropySource`
  against a real JWKS client, system clock, and CSPRNG without changing this
  crate or its tests.
- No asymmetric-crypto, HTTP, or async dependency enters the workspace until the
  API card wires the verifier.
- The required fail-closed behaviors are explicit redacted statuses backed by
  tests rather than silent acceptance.

## Alternatives considered

- Adding a JWT and JWKS crypto stack inside this crate was rejected because it
  would couple deterministic identity policy to network and cryptographic I/O
  and expand the dependency surface before any deployment consumes it.
- Keying accounts by email was rejected by the specification: email is mutable
  and non-unique across issuers, so it cannot be the durable identity.
- Importing the upstream role enum or querying its user database for admin
  status was rejected; it would couple Assay to a provider and bypass auditable
  local policy. A configured trusted-claim comparison keeps the mapping explicit
  and provider-agnostic.
- Storing the upstream id or access token in the Assay session (or returning it
  to the browser) was rejected; the browser holds only an opaque, rotatable
  session secret.
