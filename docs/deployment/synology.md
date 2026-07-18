# Deploy Assay to a Synology NAS

The production stack runs PostgreSQL 17, the Rust API and worker, and the
Next.js site. Only the web service binds a host port, on loopback. A NAS reverse
proxy is responsible for TLS and public routing.

## Safety model

- `.env.production` exists only on the NAS and is mode `600`.
- Application images deploy by registry digest; Git commit SHA remains the
  source-checkout identity, not the runtime image identity.
- PostgreSQL and web state use explicitly named external Docker volumes.
  Compose does not own those volumes, so `docker compose down -v` cannot remove
  them.
- Every deployment over an initialized database starts PostgreSQL 17 if needed,
  then creates and validates a custom-format backup before images are pulled.
- SQLx migrations are forward-only and advisory-lock serialized.
- Restore always targets a NEW external volume. The helper refuses to overwrite
  the active database volume.

## Prerequisites

Install Docker Engine, Docker Compose v2, Git, and `curl`. Give the deployment
user direct, least-privilege Docker access and ownership of the checkout. Do not
grant the workflow an interactive shell or unrestricted passwordless sudo.

Configure a reverse proxy from the production HTTPS hostname to
`http://127.0.0.1:<ASSAY_WEB_PORT>`. Do not expose the database or API ports.

## Bootstrap

Clone the public repository into an operator-chosen directory. In that checkout:

```sh
cp .env.example .env.production
chmod 600 .env.production
```

Set unique values for at least:

- `POSTGRES_PASSWORD` and the matching password inside `DATABASE_URL`
- `ASSAY_POSTGRES_VOLUME` and `ASSAY_WEB_VOLUME`
- `ASSAY_ADMISSION_HASH_KEY` when a trusted client-IP header is enabled
- `ASSAY_OLLAMA_BASE_URL`, `ASSAY_OLLAMA_MODEL`, and
  `ASSAY_OLLAMA_API_KEY` when the compatible endpoint requires a key

The Ollama base must be an HTTP(S) `/v1` base. Credentials, private NAS
addresses, reverse-proxy hostnames, and local paths must stay out of Git.

On the FIRST installation only, explicitly create the stable external volumes
and validate any existing PostgreSQL major version:

```sh
sh scripts/nas-hosted.sh --env-file .env.production init
```

## GitHub production environment

Create a protected GitHub environment named `production`.

| Kind | Name | Purpose |
| --- | --- | --- |
| Secret | `NAS_HOST` | SSH host, stored outside the repository |
| Secret | `NAS_SSH_USER` | Restricted deployment user |
| Secret | `NAS_SSH_KEY` | Private deployment key |
| Secret | `NAS_HOST_FINGERPRINT` | Expected SHA256 SSH host-key fingerprint |
| Variable | `NAS_SSH_PORT` | SSH port; defaults to `22` |
| Variable | `NAS_TARGET_DIR` | Absolute path to the existing checkout |

A push to `main` verifies Rust, web, generated contracts, Compose, and the NAS
helper. It then publishes `assay-web`, `assay-api`, and `assay-worker`, captures
the registry digest for each image, and passes only those digest references to
deployment. Deployment filters the scanned SSH keys to the exact configured
fingerprint, checks out the triggering Git SHA, and runs the fail-closed helper.
The workflow never creates, reads, transfers, or prints `.env.production`.

Routine `deploy` NEVER creates a missing configured production volume. It fails
closed and directs the operator to the explicit first-install `init` command.
This prevents a misspelled volume name from starting an empty site.

The GHCR packages must be readable by the NAS. Prefer public packages for this
public repository; otherwise configure a NAS-local read-only registry login.

## Operator commands

Run all commands from the checkout:

```sh
# Verified custom-format backup
sh scripts/nas-hosted.sh --env-file .env.production backup

# Runtime, migration-table, internal API, and loopback web checks
sh scripts/nas-hosted.sh --env-file .env.production verify

# Pull configured images, back up, migrate, start, and verify
sh scripts/nas-hosted.sh --env-file .env.production deploy
```

Backups default to `backups/postgres/`, which is ignored by Git. Copy verified
backups to separate storage with its own retention policy. A backup on the same
NAS volume is not disaster recovery.

## Restore drill

Restore a verified dump into a new external volume:

```sh
sh scripts/nas-hosted.sh --env-file .env.production restore \
  backups/postgres/assay-<timestamp>.dump assay-postgres-restore-<timestamp>
```

The helper validates the dump, refuses the active volume, starts an isolated
PostgreSQL 17 container, restores with `--exit-on-error`, and queries the SQLx
migration table. It does NOT switch production automatically.

After verification, update `ASSAY_POSTGRES_VOLUME` in `.env.production` to the
new volume and run `deploy`. Keep the previous volume until application and
data checks pass.

## PostgreSQL upgrades and rollback

`postgres:17-alpine` pins the database major. Minor PostgreSQL 17 image upgrades
use the normal backup/deploy path.

NEVER point a PostgreSQL 18 image at a PostgreSQL 17 data volume. For a major
upgrade, stop writes, create and validate a final backup, restore it into a new
volume running the new pinned major (or use a separately rehearsed
`pg_upgrade` procedure), verify the application, and only then change the
configured volume/image. Preserve the old volume for rollback.

Application rollback means selecting older API/worker/web digest references. Because
migrations are forward-only, an older binary is safe only when it is compatible
with the current schema. Otherwise restore the pre-upgrade dump into a fresh
volume and deploy the matching application SHA. Never delete or mutate the
active volume as a rollback shortcut.
