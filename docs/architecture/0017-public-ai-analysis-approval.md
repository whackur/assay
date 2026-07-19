# ADR 0017: Human approval gate for public AI analysis

- Status: Accepted
- Date: 2026-07-19

Validated judgments are not authorization to publish. Public AI analysis
requires an append-only approval naming exact evaluation and source snapshots
and recording trusted approver issuer, subject, and display name. Storage locks
the current source status, validates the exact current judgment is safe for the
existing public projection, and binds approval atomically without changing
snapshots. New source snapshots hide previous publication.

The administrative API requires a bounded bearer token. Approver identity is
accepted only in headers from the trusted web BFF after token validation; no
public route creates approvals. The public contract remains unchanged.
