use crate::{
    EvaluationErrorKind, Evaluator, EvidenceBundle, PROMPT_VERSION, ProviderRequest,
    QualitativeRubric, TransmissionSurface, api::SnapshotOutcome,
    evaluator::enforce_transmission_boundary,
};

use super::config::AgenticConfig;
use super::instructions::ControlInputs;
use super::provenance::{AgenticProvenance, AgenticSnapshot};
use super::runner::{AgentRunError, AgentRunner};
use super::workspace::{SnapshotWorkspace, WorkspaceError};

/// Agentic family adapter binding a rubric, one provider configuration, and
/// the injected snapshot-workspace and agent-runner ports.
pub struct AgenticEvaluator<W, R> {
    evaluator: Evaluator,
    config: AgenticConfig,
    workspace: W,
    runner: R,
}

impl<W: SnapshotWorkspace, R: AgentRunner> AgenticEvaluator<W, R> {
    /// Builds an adapter for one immutable rubric and provider configuration.
    pub const fn new(
        rubric: QualitativeRubric,
        config: AgenticConfig,
        workspace: W,
        runner: R,
    ) -> Self {
        Self {
            evaluator: Evaluator::new(rubric),
            config,
            workspace,
            runner,
        }
    }

    /// Returns the injected workspace port, primarily for introspection.
    pub const fn workspace(&self) -> &W {
        &self.workspace
    }

    /// Returns the injected runner port, primarily for introspection.
    pub const fn runner(&self) -> &R {
        &self.runner
    }

    /// Evaluates a bundle through one bounded agent run and always returns an
    /// explicit, recorded snapshot. Boundary enforcement gates the run before
    /// any port is touched: an external agentic provider transmits the
    /// `worktree_snapshot` surface, so the bundle's consent must have
    /// acknowledged that surface even for a public-only repository.
    pub fn evaluate(&self, bundle: &EvidenceBundle) -> AgenticSnapshot {
        let mut provenance = self.provenance(bundle);
        if let Err(error) = enforce_transmission_boundary(
            self.config.execution_boundary,
            TransmissionSurface::WorktreeSnapshot,
            bundle,
        ) {
            return failed(provenance, error.kind());
        }
        let agent = match self.runner.probe() {
            Ok(agent) => agent,
            Err(error) => return failed(provenance, run_failure(error)),
        };
        provenance.agent = Some(agent);
        let request = match ProviderRequest::new(self.evaluator.rubric(), bundle) {
            Ok(request) => request,
            Err(error) => return failed(provenance, error.kind()),
        };
        let evidence_ids = request
            .evidence()
            .iter()
            .map(|item| item.id().as_str())
            .collect::<Vec<_>>();
        let inputs = ControlInputs::new(
            request.system_instructions(),
            request.canonical_payload(),
            evidence_ids,
            &self.config.analyzed_commit,
        );
        let workspace = match self.workspace.materialize(&inputs) {
            Ok(workspace) => workspace,
            Err(error) => return failed(provenance, workspace_failure(error)),
        };
        let run = self.runner.run(&workspace);
        self.workspace.dispose(workspace);
        let run = match run {
            Ok(run) => run,
            Err(error) => return failed(provenance, run_failure(error)),
        };
        provenance.run_id = Some(run.run_id().to_owned());
        match self.evaluator.validate_bytes(run.judgment(), bundle) {
            Ok(set) => AgenticSnapshot {
                provenance,
                outcome: SnapshotOutcome::Validated(set),
            },
            Err(error) => failed(provenance, error.kind()),
        }
    }

    fn provenance(&self, bundle: &EvidenceBundle) -> AgenticProvenance {
        AgenticProvenance {
            provider_id: self.config.provider_id,
            analyzed_commit: self.config.analyzed_commit.clone(),
            model: self.config.model.clone(),
            prompt_version: PROMPT_VERSION,
            rubric_version: self.evaluator.rubric().version(),
            evaluation_version: self.evaluator.rubric().evaluation_version(),
            evidence_bundle_hash: bundle.content_hash().to_owned(),
            agent: None,
            run_id: None,
        }
    }
}

const fn failed(provenance: AgenticProvenance, kind: EvaluationErrorKind) -> AgenticSnapshot {
    AgenticSnapshot {
        provenance,
        outcome: SnapshotOutcome::Failed(kind),
    }
}

const fn workspace_failure(error: WorkspaceError) -> EvaluationErrorKind {
    match error {
        WorkspaceError::SnapshotUnavailable | WorkspaceError::ControlWriteFailed => {
            EvaluationErrorKind::ProviderFailure
        }
    }
}

const fn run_failure(error: AgentRunError) -> EvaluationErrorKind {
    match error {
        AgentRunError::ProbeFailed | AgentRunError::Failure => EvaluationErrorKind::ProviderFailure,
        AgentRunError::Timeout => EvaluationErrorKind::ProviderTimeout,
        AgentRunError::OutputTooLarge => EvaluationErrorKind::OutputTooLarge,
        AgentRunError::SandboxViolation => EvaluationErrorKind::SandboxViolation,
    }
}
