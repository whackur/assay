"use client";

import { useEffect, useState } from "react";
import type { JobState } from "@/lib/api/client";
import {
  ANALYSIS_STAGES,
  formatElapsed,
  isStageComplete,
  stageLabel,
} from "@/lib/state/stages";
import { ProjectNotice } from "@/components/ProjectNotice";

// WEB-002 progress screen: elapsed time and the current named stage, never a
// fabricated percentage. Elapsed time is measured from the job start.

export function ProgressPanel({ job }: { job: JobState }) {
  const startedAt = Date.parse(job.started_at);
  const [elapsedMs, setElapsedMs] = useState(() => Date.now() - startedAt);

  useEffect(() => {
    const timer = setInterval(() => setElapsedMs(Date.now() - startedAt), 1000);
    return () => clearInterval(timer);
  }, [startedAt]);

  return (
    <div className="stack">
      <h1>Analyzing {job.canonical.namespace}/{job.canonical.repository}</h1>
      <p className="muted">
        <a href={job.canonical_url}>{job.canonical_url}</a> · {job.profile} profile
      </p>

      <div className="card stack">
        <div>
          <span className="muted">Elapsed</span>
          <div className="elapsed" aria-live="polite">
            {formatElapsed(elapsedMs)}
          </div>
        </div>

        <ol className="stage-list">
          {ANALYSIS_STAGES.map((stage) => {
            const done = isStageComplete(stage, job.stage);
            const current = stage === job.stage;
            const cls = done ? "done" : current ? "current" : "pending";
            return (
              <li key={stage} className={`stage-item ${cls}`} aria-current={current ? "step" : undefined}>
                <span className="stage-dot" aria-hidden="true" />
                <span className="stage-name">{stageLabel(stage)}</span>
                {current && <span className="muted"> — in progress</span>}
              </li>
            );
          })}
        </ol>
      </div>

      <p className="muted">
        Completed stages are preserved. Failed stages are reported as partial or
        unavailable. There is no retry action for anonymous submissions.
      </p>
      <ProjectNotice />
    </div>
  );
}
