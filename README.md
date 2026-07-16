# Assay

Assay is an evidence-grounded open-source project intelligence dashboard. The
current web application uses versioned development fixtures while the hosted
API is being built.

## Run with Docker

Install Docker with the Compose plugin, then run from the repository root:

```sh
docker compose up --build
```

Open <http://localhost:3000>. The image builds the Next.js application as a
production standalone bundle, runs as a non-root user, and exposes a container
health check. Set `ASSAY_WEB_PORT` in `.env` to use a different host port:

```sh
cp .env.example .env
ASSAY_WEB_PORT=8080 docker compose up --build
```

Stop the stack with `docker compose down`.

## Local development

See [`web/README.md`](web/README.md) for the Next.js development, lint, type
check, test, and production build commands. See
[`crates/assay-cli/README.md`](crates/assay-cli/README.md) for the local Rust
analysis CLI contract.
