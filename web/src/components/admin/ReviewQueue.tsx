"use client";

import { useEffect, useState } from "react";
import type { ReviewQueueItem } from "@/lib/admin/review";

export function ReviewQueue({ panel }: { panel: string }) {
  const [items, setItems] = useState<ReviewQueueItem[] | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    fetch(`${panel}/api/review`, { credentials: "same-origin" })
      .then(async (response) => { if (!response.ok) throw new Error("Review queue is unavailable right now."); return response.json() as Promise<{ items: ReviewQueueItem[] }>; })
      .then((data) => setItems(data.items))
      .catch((error: unknown) => setMessage(error instanceof Error ? error.message : "Review queue is unavailable right now."));
  }, [panel]);
  async function publish(id: string) {
    setBusy(id); setMessage(null);
    const response = await fetch(`${panel}/api/review`, { method: "POST", credentials: "same-origin", headers: { "content-type": "application/json" }, body: JSON.stringify({ evaluation_snapshot_id: id }) });
    if (response.ok) { setItems((current) => current?.filter((item) => item.evaluation_snapshot_id !== id) ?? []); setMessage("Evaluation snapshot published."); }
    else { const data = await response.json().catch(() => ({})) as { error?: string }; setMessage(data.error ?? "Publishing failed. Nothing was changed."); }
    setBusy(null);
  }
  return <details className="admin-review">
    <summary>Human review queue <span className="chip">restricted</span></summary>
    <div className="admin-review-body">
      {message && <p className="notice" role="status">{message}</p>}
      {items === null && !message && <p className="muted">Loading review queue…</p>}
      {items?.length === 0 && <p className="muted">No evaluation snapshots are waiting for review.</p>}
      {items?.map((item) => <article className="review-card" key={item.evaluation_snapshot_id}>
        <header><h3>{item.analysis.data.project.owner}/{item.analysis.data.project.repository}</h3><code>{item.evaluation_snapshot_id}</code></header>
        <p><strong>Revision:</strong> <code>{item.analysis.data.revision.commit_sha}</code> ({item.analysis.data.revision.default_branch})</p>
        <div className="review-judgments">{item.analysis.data.judgments.map((judgment) => <section key={judgment.criterion_id}>
          <h4>{judgment.criterion_id}</h4><p><strong>Citations:</strong> {judgment.evidence_ids.length ? judgment.evidence_ids.join(", ") : "None"}</p><p><strong>Rationale:</strong> {judgment.rationale}</p>
        </section>)}</div>
        <button type="button" disabled={busy !== null} onClick={() => publish(item.evaluation_snapshot_id)}>{busy === item.evaluation_snapshot_id ? "Publishing…" : "Publish this evaluation snapshot"}</button>
      </article>)}
    </div>
  </details>;
}
