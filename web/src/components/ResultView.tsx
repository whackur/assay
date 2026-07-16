import type {
  ProjectComparison,
  ProjectEvaluation,
  ProjectEvidence,
} from "@/lib/contract/types";
import { resultState } from "@/lib/state/result-state";
import { ScoreCards } from "@/components/ScoreCards";
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
    <div className="stack">
      <h1>{name}</h1>
      {url && (
        <p className="muted">
          <a href={url}>{url}</a>
        </p>
      )}

      <EngineProfile evaluation={evaluation} />

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
        <section>
          <h2>Introduction</h2>
          <ul>
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

      <section>
        <h2>Dimension scores</h2>
        <p className="muted">
          The Assay Score never replaces its dimensions, and Potential is a
          separate forward-looking indicator.
        </p>
        <ScoreCards scores={evaluation.scores} />
      </section>

      <section>
        <h2>Evidence</h2>
        <EvidenceExplorer evidence={evidence} />
      </section>

      {comparison && <SimilarProjects comparison={comparison} />}

      <BadgeShare evaluation={evaluation} />

      {(evaluation.warnings.length > 0 || evaluation.limitations.length > 0) && (
        <section>
          <h2>Warnings and limitations</h2>
          <ul>
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

      <ProjectNotice />
    </div>
  );
}
