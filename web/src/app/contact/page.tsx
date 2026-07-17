import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Contact — Assay",
  description: "Report a factual or provenance concern about an Assay evaluation.",
};

// The only feedback path in the first MVP (specification 13, interview 9).
// Reactions, comments, bookmarks, follows, project claims, and formal appeals
// are deferred. Users cannot edit or retry a score.

export default function ContactPage() {
  return (
    <div className="auth-card">
      <h1>Contact and report an issue</h1>
      <div className="stack" style={{ marginTop: "var(--space-md)" }}>
        <p>
          Assay does not offer reactions, comments, bookmarks, follows, project
          claims, or a formal appeal workflow yet. It also has no user-facing
          score editing or retry.
        </p>
        <p>
          To raise a factual or provenance concern about an evaluation, contact
          the operator through the channel configured for this deployment. An
          administrator reviews reports and may correct a rule, request a manual
          re-analysis, or hide a result.
        </p>
      </div>
    </div>
  );
}
