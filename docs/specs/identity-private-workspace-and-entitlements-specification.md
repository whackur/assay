# Assay Identity, Private Workspace, and Entitlements Specification

- Status: Draft
- Product language: English
- Authentication model: Provider-agnostic OpenID Connect
- Related specification: `open-source-project-intelligence-specification.md`

## 1. Purpose

Assay supports optional member accounts without depending on any particular
website, identity implementation, user database, or deployment. An operator
connects one or more standards-compatible identity providers through
configuration.

Authentication unlocks a private Assay workspace: saved projects, unlisted
evaluations, comparisons, scheduled rescans, notifications, private notes,
exports, provider connections, and scoped Agent Skill access.

Public repository analysis remains usable without an account. Member features
extend the product but do not change the canonical public score or allow paid
or privileged users to influence it.

## 2. Independence Boundary

The open-source Assay codebase MUST NOT depend on:

- an upstream website's source repository or deployment topology;
- Firebase or any other provider-specific client SDK in the domain layer;
- an upstream user table, database connection, internal API, or role enum;
- a hard-coded issuer, domain, JWKS URL, client ID, or claim name;
- email as the durable account identifier; or
- a shared browser cookie owned by another application.

The core identity key is the validated pair:

```text
(issuer, subject)
```

An operator-specific identity system is only a configured OIDC issuer. Assay
normalizes successful identity assertions into its own account, session, role,
and entitlement records.

## 3. Authentication Architecture

### 3.1 Recommended web flow

Use an authorization-code flow with PKCE through the Assay backend-for-frontend
boundary:

```text
Browser
  -> Assay /auth/oidc/start
  -> configured identity provider
  -> Assay /auth/oidc/callback
  -> server-side code exchange and validation
  -> Assay session cookie
```

The browser receives only an opaque Assay session cookie. Upstream access and
refresh tokens remain server-side when they are required.

### 3.2 Configuration

The integration is configured at deployment time through settings equivalent
to:

```text
OIDC_ISSUER_URL
OIDC_CLIENT_ID
OIDC_CLIENT_SECRET_FILE
OIDC_REDIRECT_URI
OIDC_SCOPES=openid profile
OIDC_EXPECTED_AUDIENCE
OIDC_CLAIM_MAPPING_FILE
```

Secrets are loaded from a secret manager or mounted secret file, not committed
configuration. OIDC discovery and JWKS metadata SHOULD be used when supported.

### 3.3 Validation

Assay MUST validate:

- issuer and discovery origin;
- signature and allowed signing algorithm;
- audience and authorized party where applicable;
- expiration, not-before time, issued-at policy, and nonce;
- authorization-code transaction state and PKCE verifier;
- redirect URI against an exact allowlist; and
- subject presence and provider/account status.

Assay MUST NOT accept a token issued only for another audience. An identity
provider may issue an Assay-specific token directly or support a standards-
based token exchange for the Assay audience.

### 3.4 Session

The Assay session uses an opaque, rotated identifier in a `Secure`,
`HttpOnly`, host-only cookie with an appropriate `SameSite` policy. Session
state stores account, authentication time, assurance, expiry, and revocation.

State-changing browser requests require CSRF protection. Logout revokes the
Assay session and, when supported and explicitly intended, disconnects or
revokes the upstream session or token.

## 4. Local Authorization and Entitlements

Authentication proves identity. Assay controls authorization locally.

### 4.1 Local roles

| Role | Purpose |
| --- | --- |
| Member | Personal private workspace and normal analysis features |
| Maintainer | Verified management of a claimed project profile |
| Curator | Editorial catalog workflow without score override |
| Administrator | Assay operations and policy administration |

External roles MUST NOT automatically grant local administrative privileges.
An explicit deployment policy may map trusted external groups or claims to
local roles, and every privileged mapping is auditable.

### 4.2 Entitlements

Feature access uses entitlements rather than scattered role checks. Example
entitlements include:

```text
analysis.public.submit
analysis.private.create
analysis.compare
analysis.custom-rubric.preview
project.save
project.watch
project.claim
report.export
notification.manage
provider.codex.connect
token.agent.create
catalog.submit
catalog.curate
```

Entitlement policy is configuration- and version-controlled. Roles may bundle
entitlements, but API handlers authorize the specific action.

## 5. Member Benefits

### 5.1 Private project library — P0

Members can save public projects into private collections, attach tags and
notes, and search their history. Saved state is invisible to other users and
does not affect public popularity or project scores.

### 5.2 Unlisted evaluations — P0

Members can request an analysis whose result remains private to their
workspace. An unlisted result does not create or update a public catalog page
until the member explicitly submits it for publication and publication policy
accepts it.

The source may still be a public repository. “Private” describes the Assay
report, notes, comparison, and workflow state, not a claim that public source
has become confidential.

### 5.3 Saved comparisons and shortlists — P1

Members can compare projects within an explicit cohort, save a shortlist,
choose visible dimensions, and attach private decision notes. Custom weights
are displayed as a private scenario and never replace the canonical score.

### 5.4 Scheduled rescans and alerts — P1

Members can watch saved projects and receive notifications for:

- new releases or important repository changes;
- score or confidence changes above a configured threshold;
- evidence maturation or newly sufficient data;
- maintenance-health decline or recovery;
- newly detected claim-to-implementation mismatch; and
- catalog publication or correction decisions.

Delivery channels begin with in-app notifications and email. Webhooks or other
channels require separately scoped credentials and delivery controls.

### 5.5 Score history and private annotations — P1

Members can retain historical snapshots, annotate why a change matters, and
view which evidence, rule, rubric, or evaluator version caused a score change.
Annotations are private unless deliberately attached to a public correction or
project claim.

### 5.6 Exports and research bundles — P1

Members can export JSON, JSONL, CSV, Markdown project cards, and a reproducible
evidence manifest. Exports preserve schema, evaluation version, confidence,
limitations, and canonical source links.

### 5.7 Project ownership claim — P1

A member may claim a project through a separate proof of repository control,
such as a supported Git provider authorization or repository challenge. OIDC
membership alone does not prove repository ownership.

A verified maintainer may:

- provide project context and maturity information;
- request factual corrections or rescans;
- preview a corrected introduction;
- manage official project links; and
- see catalog moderation status.

Maintainers cannot directly edit score evidence, rules, or canonical numeric
results.

### 5.8 Personal provider connections — P1

Members can connect and disconnect supported model-provider accounts,
including the experimental Codex OAuth provider. Provider credentials remain
isolated from the identity-provider session and from Assay Agent API tokens.

### 5.9 Agent Skill and API access — P1

Members can issue scoped Assay API tokens for CLI and Agent Skill use. Tokens:

- are shown only once;
- are stored as keyed hashes;
- have explicit scopes, expiry, name, and last-used time;
- can be rotated and revoked;
- are restricted to the owning account or workspace; and
- never contain or reveal the upstream OIDC session or provider credential.

Suggested scopes:

```text
projects:read
evaluations:create
evaluations:read
collections:read
collections:write
reports:export
```

### 5.10 Optional private-repository analysis — P2

Private source analysis may be added later through a dedicated Git provider
connection. It is outside the initial public-source product and requires:

- least-privilege repository selection;
- strict source retention and deletion controls;
- isolated build and analysis workers;
- no public catalog publication;
- no comparison-corpus ingestion by default; and
- an explicit privacy and threat-model review before release.

## 6. Quota and Benefit Policy

Quotas are entitlement-driven and operator-configurable. An initial candidate
policy is:

| Identity state | New on-demand evaluations | Other benefits |
| --- | ---: | --- |
| Anonymous | 2 per IP per UTC day | Public cached results |
| Verified guest | 4 total per IP per UTC day | Access code or supported model-provider connection |
| Member | 10 per account per UTC day | Private workspace, saved history, exports |
| Member with personal model provider | Configurable higher account limit | Provider usage charged to the connected account where supported |

Member quota is account-based but remains subject to IP, repository, failure,
provider, and global service limits. Signing in does not reset already consumed
anonymous allowance; the quota ledger computes the highest applicable daily
ceiling and prior usage.

Cached results, viewing reports, managing notes, and joining an in-flight job
do not consume analysis quota. Scheduled rescans use a separate entitlement,
for example a limited number of watched projects on a weekly schedule.

Benefits and limits MUST be data-driven rather than hard-coded in the UI or
domain logic so a deployment can change policy without an application release.

## 7. Private Workspace Model

The conceptual model includes:

| Record | Purpose |
| --- | --- |
| Account | Local Assay user status and preferences |
| ExternalIdentity | Validated issuer/subject and minimal normalized claims |
| Session | Opaque browser session, expiry, rotation, and revocation |
| RoleAssignment | Local role with source and audit metadata |
| EntitlementGrant | Feature, quota, limits, validity, and policy version |
| Workspace | Personal or future team privacy boundary |
| WorkspaceMember | Workspace role and membership lifecycle |
| SavedProject | Private collection membership, tags, and notes |
| PrivateEvaluation | Workspace visibility over an immutable evaluation |
| Comparison | Saved cohort, dimensions, scenario weights, and notes |
| WatchRule | Rescan cadence and notification conditions |
| ProviderConnection | Opaque reference to separately protected provider auth |
| ApiToken | Hashed scoped token metadata and revocation state |
| QuotaLedger | Atomic reservations, consumption, release, and reset |
| AuditEvent | Security- and privacy-relevant actions without secret values |

Database queries for private records MUST be scoped by workspace before
resource lookup completes. Object identifiers alone never authorize access.

## 8. API and Frontend Contract

### 8.1 Authentication and account

```text
GET    /api/v1/auth/oidc/start
GET    /api/v1/auth/oidc/callback
POST   /api/v1/auth/logout
GET    /api/v1/me
DELETE /api/v1/me
```

The concrete provider authorization and token endpoints are discovered or
configured; they are never encoded in the public API route design.

### 8.2 Private workspace

```text
GET    /api/v1/workspaces/current
GET    /api/v1/workspaces/current/projects
POST   /api/v1/workspaces/current/projects
DELETE /api/v1/workspaces/current/projects/{project_id}
GET    /api/v1/workspaces/current/evaluations
POST   /api/v1/workspaces/current/comparisons
GET    /api/v1/workspaces/current/comparisons/{id}
POST   /api/v1/workspaces/current/watches
PATCH  /api/v1/workspaces/current/watches/{id}
DELETE /api/v1/workspaces/current/watches/{id}
```

### 8.3 Agent tokens

```text
GET    /api/v1/me/api-tokens
POST   /api/v1/me/api-tokens
DELETE /api/v1/me/api-tokens/{id}
```

The frontend displays sign-in, private/public visibility, current
entitlements, quota, saved projects, watch rules, provider connections,
security sessions, and token revocation. Sensitive values are never rendered
after initial token creation.

## 9. Agent Skill Authentication

The Agent Skill supports:

```text
assay auth login          # browser-assisted OIDC for an interactive CLI
assay auth status
assay auth logout
assay token use-env       # headless mode reads ASSAY_API_TOKEN
```

Interactive CLI login may use an OIDC device or browser authorization flow
when the configured provider supports it. Headless agents use a scoped Assay
API token supplied through a secret environment mechanism.

The skill MUST NOT read browser cookies, OIDC refresh-token storage, or model-
provider credentials. It calls the same hosted Assay API contract as any other
client and respects workspace and entitlement boundaries.

## 10. Privacy and Account Lifecycle

- Collect the minimum identity claims required for account linking.
- Do not require email when a stable subject is sufficient.
- Keep private notes, collections, comparisons, and unlisted reports out of
  public search, public APIs, analytics payloads, and comparison corpora.
- Provide session, provider connection, and API-token inspection and revocation.
- Support account export and deletion.
- Define retention separately for account records, audit events, quota
  pseudonyms, private reports, source caches, and provider connections.
- Deleting an account removes private workspace state and credentials without
  deleting immutable public facts that were independently collected from a
  public repository.
- Never let analytics or logs contain upstream tokens, session cookies, API
  tokens, private notes, or retained private source.

## 11. Functional Requirements

| ID | Pri | Requirement | Acceptance criteria |
| --- | --- | --- | --- |
| IAM-001 | P0 | Integrate human identity through generic OIDC configuration. | No domain code imports or calls a provider-specific application or user database. |
| IAM-002 | P0 | Key identities by validated issuer and subject. | Changing an email does not create a new account or merge unrelated identities. |
| IAM-003 | P0 | Enforce issuer, signature, audience, time, nonce, state, and PKCE validation. | Invalid and replayed authentication responses fail closed. |
| IAM-004 | P0 | Create an opaque Assay browser session. | Upstream tokens are absent from browser storage and application URLs. |
| IAM-005 | P0 | Authorize actions with local entitlements. | External roles do not grant Assay administration without explicit mapping policy. |
| IAM-006 | P0 | Provide member quota without login-reset abuse. | Anonymous and member usage share one atomic ledger and apply the highest eligible ceiling. |
| IAM-007 | P1 | Create scoped Agent API tokens. | Tokens are one-time-visible, hashed, expiring, revocable, and workspace-bound. |
| IAM-008 | P1 | Support account export and deletion. | Private workspace and credentials follow documented deletion and retention behavior. |
| PWS-001 | P0 | Save public projects privately. | Other users and public APIs cannot discover collection membership or notes. |
| PWS-002 | P0 | Create an unlisted evaluation. | The result remains workspace-private until an explicit catalog submission is accepted. |
| PWS-003 | P1 | Save comparisons and custom scenarios. | Private weights never overwrite canonical score data. |
| PWS-004 | P1 | Schedule rescans and notifications. | Idempotent jobs respect watch and quota entitlements and record delivery state. |
| PWS-005 | P1 | Claim project ownership separately from login. | Repository-control evidence is required before maintainer actions are granted. |
| PWS-006 | P1 | Manage personal model-provider connections. | Identity sessions, provider credentials, and Agent API tokens remain isolated credential domains. |
| PWS-007 | P2 | Analyze authorized private repositories. | Private source is isolated, retained minimally, excluded from public results, and deletable. |

## 12. Delivery Plan

### Phase A — Generic membership

- OIDC configuration, callback validation, and opaque sessions.
- Account, external identity, local role, entitlement, and quota records.
- Member dashboard and ten-evaluation candidate daily policy.
- Private saved-project collections and unlisted evaluation visibility.

### Phase B — Private workflows

- Saved comparisons, notes, score history, exports, and watch rules.
- In-app and email notifications.
- Project-claim verification and maintainer context workflow.

### Phase C — Skill and provider connections

- Scoped Assay API tokens and Agent Skill authentication.
- Personal model-provider connection management.
- Provider and session security dashboard.

### Phase D — Team and private-source expansion

- Multi-member workspaces and invitations.
- Team-scoped entitlements and audit views.
- Optional private-repository analysis after a separate threat-model review.

## 13. Security References

- OAuth 2.0 Security Best Current Practice requires protections including exact
  redirect matching, CSRF defenses, PKCE, audience restriction, and refresh-
  token replay controls: <https://www.rfc-editor.org/rfc/rfc9700.html>
- OAuth 2.0 Token Exchange defines a standards-based way to request a token for
  a target audience when an operator chooses that deployment pattern:
  <https://www.rfc-editor.org/rfc/rfc8693.html>
- OpenID Connect defines issuer, subject, audience, authorized-party, signature,
  time, and nonce validation for identity tokens:
  <https://openid.net/specs/openid-connect-core-1_0.html>
