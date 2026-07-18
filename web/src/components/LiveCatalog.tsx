"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import {
  isHostedProjectStatusResponse,
  type HostedProjectStatus,
  type HostedRecentSourceStatus,
} from "@/lib/api/hosted.generated";

type HostedListResult =
  | { state: "available"; data: HostedRecentSourceStatus[] }
  | { state: "unavailable"; reason: "api_unavailable" | "not_found" };

export function LiveSourceActivity({ result }: { result: HostedListResult }) {
  if (result.state === "unavailable") {
    return <div className="notice" role="status">Live source-processing data is unavailable. No score or empty result was inferred.</div>;
  }
  if (result.data.length === 0) {
    return <div className="notice" role="status">The hosted source queue is connected. Seed repositories have not produced a source snapshot yet.</div>;
  }
  return (
    <div className="featured-grid">
      {result.data.map((project) => (
        <article className="featured-card" key={project.provider_repository_id}>
          <div className="chip-row">
            <span className="chip accent">Source processing</span>
            <span className="chip">{project.evaluation_status ?? project.collection_status}</span>
          </div>
          <h3 className="featured-name">
            <Link href={`/projects/github/${project.owner}/${project.repository}`}>
              {project.owner}/{project.repository}
            </Link>
          </h3>
          <p className="featured-desc">
            {project.description ?? "Repository metadata was ingested; no publication decision or project score was produced."}
          </p>
          <dl className="meta">
            <dt>GitHub stars</dt><dd>{project.stars ?? "Unavailable"}</dd>
            <dt>Revision</dt><dd>{project.head_sha?.slice(0, 12) ?? "Pending"}</dd>
            <dt>Publication</dt><dd>Not evaluated</dd>
          </dl>
        </article>
      ))}
    </div>
  );
}

const TERMINAL_STATES = new Set(["complete", "partial", "unavailable"]);
const MAX_POLLS = 40;

export function LiveProjectStatus({ initial }: { initial: HostedProjectStatus }) {
  const [project, setProject] = useState(initial);
  const [pollingUnavailable, setPollingUnavailable] = useState(false);
  const totalPolls = useRef(0);

  useEffect(() => {
    if (TERMINAL_STATES.has(project.job_state) || totalPolls.current >= MAX_POLLS) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      totalPolls.current += 1;
      try {
        const query = new URLSearchParams({ owner: project.owner, repository: project.repository });
        const response = await fetch(`/api/project-evaluations?${query}`, { cache: "no-store" });
        const payload: unknown = await response.json();
        if (!response.ok || !isHostedProjectStatusResponse(payload)) throw new Error("status unavailable");
        if (cancelled) return;
        setProject(payload.data);
        setPollingUnavailable(false);
        if (!TERMINAL_STATES.has(payload.data.job_state) && totalPolls.current < MAX_POLLS) {
          timer = setTimeout(poll, 3_000);
        }
      } catch {
        if (cancelled) return;
        setPollingUnavailable(true);
        if (totalPolls.current < MAX_POLLS) timer = setTimeout(poll, 5_000);
      }
    };
    timer = setTimeout(poll, 3_000);
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [project.job_state, project.owner, project.repository]);

  return (
    <div className="featured-card">
      <h2>Workflow status</h2>
      <dl className="meta">
        <dt>Request</dt><dd>{project.request_state}</dd>
        <dt>Job</dt><dd>{project.job_state}</dd>
        <dt>Current stage</dt><dd>{project.job_stage}</dd>
        <dt>Evaluation</dt><dd>{project.evaluation_status ?? "pending"}</dd>
        <dt>Revision</dt><dd>{project.head_sha ?? "pending"}</dd>
        <dt>Project score</dt><dd>Unavailable in this workflow</dd>
      </dl>
      {project.last_error_code && <div className="notice" role="status">Stage unavailable: {project.last_error_code}. Preserved source facts remain available.</div>}
      {pollingUnavailable && <div className="notice" role="status">Live refresh is temporarily unavailable; the last known status remains visible.</div>}
    </div>
  );
}
