# Assay Repository Map

These instructions apply across the repository. A more specific `AGENTS.md` in a
subtree would inherit and refine them; no subtree-specific files are defined by
this map.

## Authority

- Product specifications in [`docs/specs/`](docs/specs/) are the implementation
  source of truth.
- Handoffs in [`docs/internal/handoffs/`](docs/internal/handoffs/) are
  point-in-time context and never override specifications.
- The complete working rules are in
  [`docs/development/agent-instructions.md`](docs/development/agent-instructions.md).
  Read and obey that document for every change.

## Work locations

| Work | Location |
| --- | --- |
| Core domain, collection, classification, diff, metrics, storage, identity, project intelligence, and AI evaluation | `crates/` (see the detailed ownership map) |
| Thin API, worker, and Codex broker applications | `apps/` |
| CLI and Agent Skill | `crates/assay-cli`, `skills/assay` |
| Rust-contract-driven web UI | `web/` |
| Specifications and architecture decisions | `docs/specs/`, `docs/architecture/` |

## Ownership at a glance

| Area | Owner |
| --- | --- |
| Domain, Git, GitHub, classification, semantic diff, metrics, storage | `crates/assay-domain`, `assay-git`, `assay-github`, `assay-classifier`, `assay-semantic-diff`, `assay-metrics`, `assay-storage` |
| Identity and project intelligence | `crates/assay-identity`, `crates/assay-project-intelligence` |
| AI evaluation | `crates/assay-ai-evaluator` |
| Public Codex authorization and isolated execution | `apps/assay-codex-broker` |

For dependency boundaries, product boundaries, testing, security, contracts, and
file-size rules, follow the [detailed agent instructions](docs/development/agent-instructions.md).
