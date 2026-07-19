use crate::run::error::{RunError, RunErrorKind};
use crate::run::id::RunId;
use crate::run::lifecycle::{AdminAction, AdminAuditEvent, Administrator, RunLifecycle};
use crate::run::mapping::run_machine_value;
use crate::run::policy::RetryPolicy;
use crate::run::stage::{AttemptDisposition, PIPELINE_STAGES, Stage, StageAttempt, StageStatus};
use crate::run::state::StageState;
use crate::run::validation::{validate_reason, validate_timestamp};

/// One project analysis run modeled as a preserving stage state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRun {
    pub(crate) run_id: RunId,
    pub(crate) lifecycle: RunLifecycle,
    pub(crate) policy: RetryPolicy,
    pub(crate) stages: Vec<StageState>,
    pub(crate) audit: Vec<AdminAuditEvent>,
}

impl ProjectRun {
    /// Starts a run with every stage pending under the given retry policy.
    pub fn new(run_id: RunId, policy: RetryPolicy) -> Self {
        Self {
            run_id,
            lifecycle: RunLifecycle::Active,
            policy,
            stages: PIPELINE_STAGES
                .iter()
                .map(|&s| StageState::pending(s))
                .collect(),
            audit: Vec::new(),
        }
    }

    /// Returns the run identifier.
    pub const fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Returns the run record lifecycle.
    pub const fn lifecycle(&self) -> RunLifecycle {
        self.lifecycle
    }

    /// Returns the bounded-retry policy governing automatic retries.
    pub const fn policy(&self) -> RetryPolicy {
        self.policy
    }

    /// Returns the current status of one stage.
    pub fn stage_status(&self, stage: Stage) -> StageStatus {
        self.state(stage).status
    }

    /// Returns the number of automatic attempts recorded for one stage.
    pub fn stage_attempts(&self, stage: Stage) -> u32 {
        self.state(stage).attempts
    }

    /// Returns the redacted failure reason for one stage, when it has failed.
    pub fn stage_reason(&self, stage: Stage) -> Option<&str> {
        self.state(stage).reason.as_deref()
    }

    /// Reports whether one stage exhausted its automatic retry budget.
    pub fn stage_retries_exhausted(&self, stage: Stage) -> bool {
        self.state(stage).status == StageStatus::Unavailable
    }

    /// Ordinary users can never retry: recovery is administrator-only by design.
    pub const fn ordinary_user_retry_available(&self) -> bool {
        false
    }

    /// Returns the audit trail of administrator actions in recording order.
    pub fn audit_events(&self) -> &[AdminAuditEvent] {
        &self.audit
    }

    /// Derives the run status; a mixed run is `partial`, never a false success.
    pub fn status(&self) -> StageStatus {
        let mut all_complete = true;
        let mut any_usable = false;
        let mut any_unavailable = false;
        for state in &self.stages {
            match state.status {
                StageStatus::Complete => any_usable = true,
                StageStatus::Partial => {
                    all_complete = false;
                    any_usable = true;
                }
                StageStatus::Unavailable => {
                    all_complete = false;
                    any_unavailable = true;
                }
                StageStatus::Pending => all_complete = false,
            }
        }
        if all_complete {
            StageStatus::Complete
        } else if any_usable {
            StageStatus::Partial
        } else if any_unavailable {
            StageStatus::Unavailable
        } else {
            StageStatus::Pending
        }
    }

    /// Records one bounded worker attempt and settles the stage per policy.
    pub fn record_attempt(
        &mut self,
        stage: Stage,
        attempt: StageAttempt,
    ) -> Result<AttemptDisposition, RunError> {
        if self.lifecycle != RunLifecycle::Active {
            return Err(RunError::new(RunErrorKind::RunNotActive));
        }
        if self.state(stage).status.is_terminal() {
            return Err(RunError::new(RunErrorKind::StageNotPending));
        }
        match attempt {
            StageAttempt::Completed(snapshot) => {
                let state = self.state_mut(stage);
                state.attempts += 1;
                state.status = StageStatus::Complete;
                state.reason = None;
                state.result_snapshot = Some(snapshot);
                Ok(AttemptDisposition::Settled)
            }
            StageAttempt::PartiallyCompleted { snapshot, reason } => {
                let reason = validate_reason(&reason)?;
                let state = self.state_mut(stage);
                state.attempts += 1;
                state.status = StageStatus::Partial;
                state.reason = Some(reason);
                state.result_snapshot = Some(snapshot);
                Ok(AttemptDisposition::Settled)
            }
            StageAttempt::Failed { reason } => {
                let reason = validate_reason(&reason)?;
                let max_attempts = self.policy.max_attempts();
                let state = self.state_mut(stage);
                state.attempts += 1;
                if state.attempts >= max_attempts {
                    state.status = StageStatus::Unavailable;
                    state.reason = Some(reason);
                    state.result_snapshot = None;
                    Ok(AttemptDisposition::Exhausted)
                } else {
                    state.status = StageStatus::Pending;
                    state.reason = None;
                    state.result_snapshot = None;
                    Ok(AttemptDisposition::RetryScheduled)
                }
            }
        }
    }

    /// Reruns one failed stage, preserving every completed stage's snapshot.
    pub fn rerun_stage(
        &mut self,
        stage: Stage,
        _administrator: &Administrator,
        at: &str,
    ) -> Result<AdminAuditEvent, RunError> {
        self.require_active()?;
        let recorded_at = validate_timestamp(at)?;
        if !self.state(stage).status.is_failed() {
            return Err(RunError::new(RunErrorKind::StageNotFailed));
        }
        self.state_mut(stage).reset();
        Ok(self.record_admin(AdminAction::RerunStage, Some(stage), recorded_at))
    }

    /// Reruns every failed stage, reusing all completed immutable snapshots.
    pub fn rerun_failed_stages(
        &mut self,
        _administrator: &Administrator,
        at: &str,
    ) -> Result<AdminAuditEvent, RunError> {
        self.require_active()?;
        let recorded_at = validate_timestamp(at)?;
        let failed: Vec<Stage> = self
            .stages
            .iter()
            .filter(|state| state.status.is_failed())
            .map(|state| state.stage)
            .collect();
        if failed.is_empty() {
            return Err(RunError::new(RunErrorKind::NothingToRerun));
        }
        for stage in failed {
            self.state_mut(stage).reset();
        }
        Ok(self.record_admin(AdminAction::RerunFailedStages, None, recorded_at))
    }

    /// Soft-deletes the run; completed snapshots are retained for restoration.
    pub fn soft_delete(
        &mut self,
        _administrator: &Administrator,
        at: &str,
    ) -> Result<AdminAuditEvent, RunError> {
        let recorded_at = validate_timestamp(at)?;
        if self.lifecycle != RunLifecycle::Active {
            return Err(RunError::new(RunErrorKind::InvalidLifecycleTransition));
        }
        self.lifecycle = RunLifecycle::Deleted;
        Ok(self.record_admin(AdminAction::SoftDelete, None, recorded_at))
    }

    /// Restores a soft-deleted run to active.
    pub fn restore(
        &mut self,
        _administrator: &Administrator,
        at: &str,
    ) -> Result<AdminAuditEvent, RunError> {
        let recorded_at = validate_timestamp(at)?;
        if self.lifecycle != RunLifecycle::Deleted {
            return Err(RunError::new(RunErrorKind::InvalidLifecycleTransition));
        }
        self.lifecycle = RunLifecycle::Active;
        Ok(self.record_admin(AdminAction::Restore, None, recorded_at))
    }

    /// Purges the run, irrecoverably dropping stage result content.
    ///
    /// The audit trail is retained; only produced content is removed.
    pub fn purge(
        &mut self,
        _administrator: &Administrator,
        at: &str,
    ) -> Result<AdminAuditEvent, RunError> {
        let recorded_at = validate_timestamp(at)?;
        if self.lifecycle == RunLifecycle::Purged {
            return Err(RunError::new(RunErrorKind::InvalidLifecycleTransition));
        }
        self.lifecycle = RunLifecycle::Purged;
        for state in &mut self.stages {
            state.result_snapshot = None;
        }
        Ok(self.record_admin(AdminAction::Purge, None, recorded_at))
    }

    /// Maps the run onto `schemas/run-state/v1.json`.
    pub fn to_machine_value(&self) -> serde_json::Value {
        run_machine_value(
            &self.run_id,
            self.lifecycle,
            self.status(),
            self.ordinary_user_retry_available(),
            self.policy,
            &self.stages,
            &self.audit,
        )
    }

    fn require_active(&self) -> Result<(), RunError> {
        if self.lifecycle == RunLifecycle::Active {
            Ok(())
        } else {
            Err(RunError::new(RunErrorKind::RunNotActive))
        }
    }

    fn record_admin(
        &mut self,
        action: AdminAction,
        stage: Option<Stage>,
        recorded_at: String,
    ) -> AdminAuditEvent {
        let event = AdminAuditEvent {
            action,
            run_id: self.run_id.clone(),
            stage,
            policy_version: self.policy.version,
            recorded_at,
        };
        self.audit.push(event.clone());
        event
    }

    fn state(&self, stage: Stage) -> &StageState {
        self.stages
            .iter()
            .find(|state| state.stage == stage)
            .expect("every pipeline stage is present")
    }

    fn state_mut(&mut self, stage: Stage) -> &mut StageState {
        self.stages
            .iter_mut()
            .find(|state| state.stage == stage)
            .expect("every pipeline stage is present")
    }
}
