import type {
  ProjectComparison,
  ProjectEvaluation,
  ProjectEvidence,
} from "@/lib/contract/types";
import { resultState } from "@/lib/state/result-state";
import { ScoreViews } from "@/components/ScoreViews";
import { EngineProfile } from "@/components/EngineProfile";
import { EvidenceExplorer } from "@/components/EvidenceExplorer";
import { SimilarProjects } from "@/components/SimilarProjects";
import { BadgeShare } from "@/components/BadgeShare";
import { ProjectNotice } from "@/components/ProjectNotice";

function repositoryUrl(evaluation: ProjectEvaluation): string | null {
  const source = evaluation.project.source;
  if (source.kind !== "hosted") return null;
  return `https://${source.provider}.com/${source.namespace}/${source.repository}`;
}

export function ResultView({
  evaluation,
  evidence,
  comparison = null,
}: {
  evaluation: ProjectEvaluation;
  evidence: ProjectEvidence[];
  comparison?: ProjectComparison | null;
}) {
  const state = resultState(evaluation);
  const source = evaluation.project.source;
  const name =
    source.kind === "hosted"
      ? `${source.namespace}/${source.repository}`
      : source.repository_id;
  const url = repositoryUrl(evaluation);
  const { introduction } = evaluation;

  return (
    <article>
      <header className="report-masthead">
        <h1>{name}</h1>
        {url && (
          <p className="report-source">
            <a href={url}>{url}</a> · revision{" "}
            {evaluation.project.revision.slice(0, 12)}
          </p>
        )}
        <EngineProfile evaluation={evaluation} />
      </header>

      {(state.isPartial || state.providerUnavailable || !state.releaseGateMet) && (
        <p className="notice" role="status">
          {state.providerUnavailable
            ? "An evaluation provider was unavailable, so some dimensions are partial. "
            : ""}
          {!state.releaseGateMet
            ? "The public score release gate is not met; the Assay Score stays unavailable rather than showing a misleading zero. "
            : ""}
          There is no user-controlled retry.
        </p>
      )}

      {introduction.status === "complete" && (
        <section className="report-section" aria-labelledby="report-intro">
          <h2 id="report-intro">Introduction</h2>
          <ul className="intro-list">
            {introduction.factual_statements.map((s, i) => (
              <li key={`fact-${i}`}>{s.text}</li>
            ))}
            {introduction.interpretations.map((s, i) => (
              <li key={`interp-${i}`} className="muted">
                {s.text}
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="report-section" aria-labelledby="report-scores">
        <h2 id="report-scores">Scores</h2>
        <p className="lede">
          The Assay Score never replaces its dimensions, and Potential is a
          separate forward-looking indicator.
        </p>
        <ScoreViews scores={evaluation.scores} />
        <aside className="hakhub-cta" aria-label="HakHub teaser">
          <div className="hakhub-cta-text">
            <p className="hakhub-cta-heading">Want more from your score?</p>
            <p className="hakhub-cta-lede">
              See how projects like this one stack up across the wider
              ecosystem.
            </p>
          </div>
          <a
            className="hakhub-cta-link"
            href="https://hakhub.net/"
            target="_blank"
            rel="noopener noreferrer"
          >
            Explore HakHub
          </a>
        </aside>
      </section>

      <section className="report-section" aria-labelledby="report-evidence">
        <h2 id="report-evidence">Evidence</h2>
        <p className="lede">
          Every record below is graded, revision-pinned, and citable by the
          scores above.
        </p>
        <EvidenceExplorer evidence={evidence} />
      </section>

      {comparison && <SimilarProjects comparison={comparison} />}

      <BadgeShare evaluation={evaluation} />

      {(evaluation.warnings.length > 0 || evaluation.limitations.length > 0) && (
        <section className="report-section" aria-labelledby="report-limits">
          <h2 id="report-limits">Warnings and limitations</h2>
          <ul className="intro-list">
            {evaluation.warnings.map((w) => (
              <li key={`w-${w.code}`}>
                <code>{w.code}</code>
              </li>
            ))}
            {evaluation.limitations.map((l) => (
              <li key={`l-${l.code}`} className="muted">
                <code>{l.code}</code>
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="report-section">
        <ProjectNotice />
      </div>
    </article>
  );
}
