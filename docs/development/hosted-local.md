# Run the hosted vertical slice

This stack admits public GitHub repositories, preserves normalized source facts
in PostgreSQL, validates optional Ollama-compatible qualitative output, and
shows live source-processing state in the existing Next.js site. It is a local
hosted-development stack, not the production NAS deployment contract.

## Quick path

1. Copy `.env.example` to `.env` and set `POSTGRES_PASSWORD` and the matching
   password inside `DATABASE_URL`.
2. Set `ASSAY_OLLAMA_MODEL`. For a remote compatible service, also set its
   HTTPS `/v1` base and `ASSAY_OLLAMA_API_KEY`.
3. Run `docker compose -f compose.hosted.yaml up --build`.
4. Open `http://localhost:3000`. The default seed is `whackur/assay`.

The GitHub token is optional for public repositories. If it is present, use a
read-only least-privilege token. Assay never accepts GitHub or provider tokens
through the public submission form.

The GitHub transport is owned by `assay-github`. The Ollama request, bounded
response handling, provider provenance, and canonical evidence-citation
validation are owned by `assay-ai-evaluator`. Shared source-status contracts,
projection policy, and workflow sequencing are owned by
`assay-project-intelligence`. `assay-worker` only parses operator configuration,
wires those ports to storage, and drives the shared workflow.

Anonymous admission is bounded independently by a one-way client bucket, a
repository-owner bucket, provider burst/circuit limits, and
`ASSAY_MAX_ACTIVE_JOBS`. Forwarded headers are not trusted by default. An
operator behind a sanitizing proxy may configure
`ASSAY_TRUSTED_CLIENT_IP_HEADER` plus `ASSAY_ADMISSION_HASH_KEY`; otherwise all
anonymous requests safely share one bucket. Raw client addresses never reach
the Rust API. Completed projects use
`ASSAY_PUBLIC_COMPLETED_COOLDOWN_SECONDS`; unavailable terminal jobs may be
resubmitted after `ASSAY_PUBLIC_FAILURE_BACKOFF_SECONDS`. Active duplicates join
the existing job instead of consuming another reservation.

## Data safety

The local `assay-postgres-data` volume is Compose-managed development data.
Production instead uses an explicitly named external volume and the fail-closed
helper documented in `docs/deployment/synology.md`.

> **Destructive:** `docker compose -f compose.hosted.yaml down -v` deletes the
> PostgreSQL volume and its history. Use `docker compose -f
> compose.hosted.yaml down` without `-v` for an ordinary shutdown.

## Honest partial states

If GitHub is unavailable, the repository remains queued or partial with a
stage error and bounded retries preserve every attempt. If the Ollama endpoint,
key, model, or OpenAI-compatible contract is unavailable, collected GitHub
facts remain durable and evaluation retries resume from the exact stored source
snapshot. Provider Retry-After/reset timing is honored up to
`ASSAY_PROVIDER_RETRY_MAX_SECONDS`. A canonically validated provider response is
recorded as `validated_unpublished`, not as a failure: provider text and ratings
are discarded, bounded attempt provenance is retained, and no score or
publication approval is exposed.

The home page read model is recent ingested source status. It is NOT the Assay
project catalog, a ranking, or evidence that a repository passed publication
policy. It remains hidden from publication until an explicit catalog policy is
added.

## Contract checks

The internal route payload is the versioned `assay-hosted-api` `1.0.0`
contract under `schemas/hosted-api/1.0.0.json`. Regenerate and verify the web
types after a schema change:

```sh
node scripts/generate-hosted-contract.mjs
node scripts/generate-hosted-contract.mjs --check
cargo test -p assay-project-intelligence --test hosted_contract
```

The route remains deployment-internal. The version number describes the exact
payload contract and does not advertise the route as a general public service.
