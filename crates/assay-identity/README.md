# assay-identity

This crate owns Assay's provider-agnostic identity boundary: OIDC token
validation, the authorization-code flow, opaque sessions, local roles and
entitlements, and the explicit mapping of a trusted upstream claim to the Assay
Administrator role. It depends only on `serde` and `sha2` and performs no
network, filesystem, process, database, or wall-clock I/O. Signature and JWKS
crypto, the clock, and entropy are injected through narrow ports, so all
validation, session, and policy logic is deterministic and fully testable.

## Boundary

The core keys identities by the validated `(issuer, subject)` pair, never by
email. The issuer, client id, audience, redirect allowlist, allowed algorithms,
and any trusted admin claim are configuration values on `OidcDeploymentConfig`.
No provider domain, private claim convention, or role enum is compiled in, and
the crate never queries an upstream user database. A single deployment example
lives in `configs/identity-single-issuer.example.toml`; it holds no credentials.

## Validation

`TokenValidator` fails closed on issuer, algorithm, audience and authorized
party, expiration, not-before, issued-at, nonce, and subject checks, returning a
redacted `ValidationError`. A token minted only for another audience is
rejected. The `SigningAlgorithm` enum cannot represent `none` or symmetric HMAC.
The `SignatureVerifier` port proves the signature over the issuer JWKS and
reports the algorithm; it enforces no policy. The concrete verifier is injected
by a deployment and is exercised in tests by a deterministic fake.

## Flow and session

`AuthorizationStore` issues and single-uses `state`, `nonce`, and a PKCE `S256`
verifier; a reused or unknown state and a non-allowlisted or mismatched redirect
fail closed. `Session` is an opaque, rotatable, revocable, expiring record bound
to an account key. It never stores an upstream token, and the browser holds only
a redacted cookie secret.

## Authorization and audit

Authorization is local. `EntitlementPolicy` maps `LocalRole` to `Entitlement`
bundles that handlers check per action; external roles never elevate on their
own. `AdministratorMappingPolicy` grants Administrator only for an explicitly
configured `TrustedAdminClaim`, comparing claim values as opaque strings. Every
privileged mapping emits an `AuditEvent` recording the action, account key,
matched claim name, and policy version, and no secret value.

## Secret handling

`UpstreamIdToken`, `State`, `Nonce`, `PkceVerifier`, and `SessionSecret` redact
their `Debug`, implement no `Display`, and derive no `Serialize`; each exposes
its value only through a named `reveal`. Errors carry a stable kind and reason
but never the rejected value.

The boundary decisions are recorded in
`docs/architecture/0009-identity-boundary-and-signature-verification-port.md`.
