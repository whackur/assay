# Deploy the Assay Preview to Synology NAS

This deployment publishes the current fixture-backed web preview. It is not a
production-ready hosted Assay service: security hardening and the API, worker,
database, identity, and AI-provider integrations are intentionally deferred.

## Architecture

1. A push to `main` builds the root `Dockerfile` target `web-runtime` in GitHub
   Actions.
2. GitHub Actions publishes `ghcr.io/whackur/assay-web` with `latest` and the
   triggering commit SHA.
3. The deploy job connects to the NAS, resets the public checkout to that
   `main` commit, and starts the immutable SHA image with Compose project
   `assay-prod`.
4. Compose exposes the container only at `127.0.0.1:3001`; the NAS reverse
   proxy terminates TLS and serves the public hostname.

Persistent preview state is stored in the Compose-managed
`assay-prod_assay-web-data` volume. The deploy workflow never reads or writes
`.env` files.

## Prerequisites

- Point `assay.hakhub.net` DNS at the NAS and issue a certificate that covers
  that hostname.
- Configure the NAS reverse proxy for `assay.hakhub.net` to forward HTTPS
  traffic to `http://127.0.0.1:3001`, preserving the host and forwarding
  headers.
- Install Docker Engine, Docker Compose v2, Git, and `curl` on the NAS.
- Ensure the SSH deployment user owns the checkout and has passwordless,
  non-interactive sudo access for the workflow's `env ... docker` and cleanup
  commands. Direct Docker access is not available; every Docker command uses
  `sudo -n`.
- Use `/volume1/docker/project/assay` as the deployment path.

## GitHub production environment

Create a GitHub environment named `production` with these secrets. Store only
their values in GitHub; never commit them.

| Secret | Purpose |
| --- | --- |
| `REMOTE_HOST` | NAS SSH host |
| `REMOTE_USER` | NAS deployment user |
| `REMOTE_PORT` | NAS SSH port |
| `REMOTE_KEY` | Private SSH key for the deployment user |
| `REMOTE_FINGERPRINT` | SHA256 fingerprint of the NAS SSH host key |
| `TARGET_DIR` | `/volume1/docker/project/assay` |

The workflow-scoped `GITHUB_TOKEN` authenticates to GHCR only through a
temporary Docker configuration. Login and logout run through `sudo -n`, and a
trap uses `sudo -n rm -rf` to remove root-owned credential files. The public Git
remote never contains credentials.

## Bootstrap the NAS checkout

Run once as the deployment user:

```sh
mkdir -p /volume1/docker/project/assay
git clone https://github.com/whackur/assay.git /volume1/docker/project/assay
git -C /volume1/docker/project/assay checkout main
```

Do not create a deployment `.env` file. The workflow supplies `ASSAY_IMAGE` and
`ASSAY_WEB_PORT` only to the Compose process. The target directory must already
exist and contain this public checkout before the first deployment.

## Deploy and verify

Push to `main` or run **Deploy Assay Preview to Synology NAS** manually. The
workflow publishes both tags but deploys only
`ghcr.io/whackur/assay-web:<github.sha>`; `latest` is a convenience tag, not the
deployment source of truth.

On the NAS, verify:

```sh
cd /volume1/docker/project/assay
ASSAY_IMAGE="${ASSAY_IMAGE:-ghcr.io/whackur/assay-web:latest}"
ASSAY_WEB_PORT=3001
sudo -n env \
  COMPOSE_DISABLE_ENV_FILE=1 \
  ASSAY_IMAGE="$ASSAY_IMAGE" \
  ASSAY_WEB_PORT="$ASSAY_WEB_PORT" \
  docker compose -f compose.yaml ps web
curl --fail --show-error http://127.0.0.1:3001/
```

The container must report `healthy`, and the loopback smoke request must
succeed. Complete verification through the configured HTTPS hostname after DNS,
certificate, and reverse-proxy setup.

## Rollback

Before replacement, the workflow records the image of the running
`assay-prod` web container. If pull, startup, health, or smoke verification
fails, it makes one automatic attempt to recreate the service with that image.
The workflow still fails so the incident remains visible.

For a manual rollback, select a previously published commit SHA and run without
an `.env` file:

```sh
cd /volume1/docker/project/assay
ASSAY_IMAGE=ghcr.io/whackur/assay-web:<previous-sha>
ASSAY_WEB_PORT=3001
sudo -n env \
  COMPOSE_DISABLE_ENV_FILE=1 \
  ASSAY_IMAGE="$ASSAY_IMAGE" \
  ASSAY_WEB_PORT="$ASSAY_WEB_PORT" \
  docker compose -f compose.yaml up -d --no-build --force-recreate web
```

Then repeat the health and loopback smoke checks.

## Admin authentication modes

The admin area always lives under the secret per-deployment path
`/panel-<slug>` (stored server-side in `<data dir>/admin.json`). How the
operator authenticates behind that path is selected by environment:

- **Standalone (default).** No configuration required. First boot prints a
  one-time setup URL to the server console; the operator creates a local
  username/password admin, and sessions are stored in `admin.json`. This is
  what a fresh `git clone` gets.
- **SSO.** Set `ASSAY_SSO_JWKS_URL` to trust an external identity provider
  (e.g. hakhub.net) instead. The admin identity is an RS256 JWT read from a
  cookie the IdP sets on the shared parent domain (`domain=.hakhub.net`, so it
  arrives on `assay.hakhub.net`). The token is verified server-side against
  the JWKS with a pinned issuer, and its `roles` claim must contain the admin
  role. Local setup, login, and logout are plain 404s; `admin.json`
  credentials and sessions are ignored for auth (the file still supplies the
  panel slug). Unauthenticated admin pages redirect to `ASSAY_SSO_LOGIN_URL`
  with a `returnUrl` query parameter when it is set, and otherwise render the
  same 404 as a wrong slug; admin API routes return `401` JSON.

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `ASSAY_SSO_JWKS_URL` | enables SSO mode | unset (standalone) | JWKS endpoint of the identity provider |
| `ASSAY_SSO_ISSUER` | yes, in SSO mode | — | Expected `iss` claim; without it all SSO logins are refused (fail closed) |
| `ASSAY_SSO_AUDIENCE` | no | unset | Expected `aud` claim; verified only when set |
| `ASSAY_SSO_COOKIE` | no | `access_token` | Cookie name carrying the JWT |
| `ASSAY_SSO_ADMIN_ROLE` | no | `admin` | Role (in the token's `roles` array) that grants admin access |
| `ASSAY_SSO_LOGIN_URL` | no | unset | IdP sign-in page for unauthenticated admin page redirects |

## Known risks

- The preview uses fixture-backed behavior; hosted API, worker, database,
  identity, and AI-provider paths are not deployed.
- Initial admin setup is not yet protected, and login rate limiting is absent.
- Admin persistence is single-process file-backed state in one Docker volume;
  backup and restore procedures are not yet automated.
- Public exposure depends on correct NAS firewall, DNS, TLS, and reverse-proxy
  configuration.

These risks are accepted only for the current preview. Security hardening is a
separate required workstream before treating Assay as a production service.
