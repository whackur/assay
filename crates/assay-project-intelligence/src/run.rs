//! Project-intelligence run orchestration and administrator recovery.
//!
//! Models the named analysis pipeline as a stage state machine. A partial stage
//! failure never fails the whole run: completed stages keep their immutable
//! result snapshot while only failed stages carry `partial` or `unavailable`
//! plus a redacted reason. The system retries a failed stage a bounded,
//! versioned number of times; once that budget is spent there is no ordinary
//! user retry path. Only an administrator capability may rerun failed stages,
//! soft delete, restore, or purge a run, and every such action appends a
//! secret-free audit event.
//!
//! The four-state lifecycle vocabulary (`pending`, `complete`, `partial`,
//! `unavailable`) deliberately mirrors the domain availability states without
//! importing them: a stage status is a pipeline position, not an evidence fact,
//! so `unavailable` and `partial` are never disguised as a zero or a success.
//!
//! No clock, filesystem, process, or network I/O: timestamps and identifiers
//! are injected, so identical input yields byte-identical output.

use std::{error::Error, fmt};

use assay_domain::ContentHash;
use serde_json::{Value, json};

const RUN_STATE_VERSION: &str = "project-run-1";
const SCHEMA_VERSION: &str = "1.0.0";
const MAX_TIMESTAMP_BYTES: usize = 64;

/// One named stage of the analysis pipeline, in interview-defined order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Stage {
    SourceVerification,
    RevisionPinning,
    FileAndHistoryAnalysis,
    ProjectTypeDetermination,
    CiAndDependencyEvidence,
    SimilarProjectDiscovery,
    AiRubricEvaluation,
    ScoreCompilation,
    ResultPublication,
}

/// The named pipeline stages in canonical execution order.
pub const PIPELINE_STAGES: [Stage; 9] = [
    Stage::SourceVerification,
    Stage::RevisionPinning,
    Stage::FileAndHistoryAnalysis,
    Stage::ProjectTypeDetermination,
    Stage::CiAndDependencyEvidence,
    Stage::SimilarProjectDiscovery,
    Stage::AiRubricEvaluation,
    Stage::ScoreCompilation,
    Stage::ResultPublication,
];

impl Stage {
    /// Returns the stable machine field name used in the public contract.
    pub const fn code(self) -> &'static str {
        match self {
            Self::SourceVerification => "source_verification",
            Self::RevisionPinning => "revision_pinning",
            Self::FileAndHistoryAnalysis => "file_and_history_analysis",
            Self::ProjectTypeDetermination => "project_type_determination",
            Self::CiAndDependencyEvidence => "ci_and_dependency_evidence",
            Self::SimilarProjectDiscovery => "similar_project_discovery",
            Self::AiRubricEvaluation => "ai_rubric_evaluation",
            Self::ScoreCompilation => "score_compilation",
            Self::ResultPublication => "result_publication",
        }
    }
}

/// The four-state lifecycle position of a stage or of the whole run.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StageStatus {
    /// Not yet settled: never attempted, or failed with retry budget remaining.
    Pending,
    /// Settled with a complete, reusable result snapshot.
    Complete,
    /// Settled with a usable result that has explicit gaps.
    Partial,
    /// Settled without a usable result after the retry budget was exhausted.
    Unavailable,
}

impl StageStatus {
    const fn code(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Partial | Self::Unavailable)
    }

    const fn is_failed(self) -> bool {
        matches!(self, Self::Partial | Self::Unavailable)
    }
}

/// The record lifecycle of a whole run: soft deletion mirrors the local journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunLifecycle {
    Active,
    Deleted,
    Purged,
}

impl RunLifecycle {
    const fn code(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
            Self::Purged => "purged",
        }
    }
}

/// The outcome a worker reports for one bounded attempt at a stage.
#[derive(Clone, Debug, PartialEq)]
pub enum StageAttempt {
    /// The stage produced a complete, reusable result snapshot.
    Completed(ContentHash),
    /// The stage produced a usable result with explicit gaps and a reason.
    PartiallyCompleted {
        snapshot: ContentHash,
        reason: String,
    },
    /// The stage failed to produce a usable result, with a redacted reason.
    Failed { reason: String },
}

/// How the state machine settled a recorded attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptDisposition {
    /// The stage reached a terminal state (`complete` or `partial`).
    Settled,
    /// The stage failed but automatic retry budget remains; it stays `pending`.
    RetryScheduled,
    /// The stage failed and the automatic retry budget is now exhausted.
    Exhausted,
}

/// A privileged action recorded on a run, gated by an administrator capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminAction {
    RerunStage,
    RerunFailedStages,
    SoftDelete,
    Restore,
    Purge,
}

impl AdminAction {
    const fn code(self) -> &'static str {
        match self {
            Self::RerunStage => "rerun_stage",
            Self::RerunFailedStages => "rerun_failed_stages",
            Self::SoftDelete => "soft_delete",
            Self::Restore => "restore",
            Self::Purge => "purge",
        }
    }
}

/// A capability held only by an administrator.
///
/// Presenting this token is the gate for every recovery operation. The identity
/// layer decides who may assume it (its `analysis.admin.*` entitlements); this
/// crate stays free of any provider or role source.
#[derive(Clone, Copy, Debug)]
pub struct Administrator(());

impl Administrator {
    /// Assumes the administrator role for a caller the identity layer authorized.
    pub const fn assume() -> Self {
        Self(())
    }
}

/// A secret-free audit record for one administrator recovery action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminAuditEvent {
    action: AdminAction,
    run_id: RunId,
    stage: Option<Stage>,
    policy_version: &'static str,
    recorded_at: String,
}

impl AdminAuditEvent {
    /// Returns the recorded privileged action.
    pub const fn action(&self) -> AdminAction {
        self.action
    }

    /// Returns the run the action applied to.
    pub const fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Returns the single targeted stage, when the action was a stage rerun.
    pub const fn stage(&self) -> Option<Stage> {
        self.stage
    }

    /// Returns the injected timestamp.
    pub fn recorded_at(&self) -> &str {
        &self.recorded_at
    }

    fn to_value(&self) -> Value {
        json!({
            "action": self.action.code(),
            "run_id": self.run_id.as_str(),
            "stage": self.stage.map(Stage::code),
            "policy_version": self.policy_version,
            "recorded_at": self.recorded_at,
        })
    }
}

/// A stable, redacted run-operation failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunErrorKind {
    InvalidRunId,
    InvalidReason,
    InvalidTimestamp,
    /// A recording or rerun was attempted on a run that is not active.
    RunNotActive,
    /// An attempt was recorded on a stage that is already terminal.
    StageNotPending,
    /// A stage rerun targeted a stage that has not failed.
    StageNotFailed,
    /// A failed-stage rerun found no failed stage to rerun.
    NothingToRerun,
    /// A lifecycle transition was requested from an incompatible state.
    InvalidLifecycleTransition,
}

/// A redacted run-operation failure that never echoes source or path material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunError {
    kind: RunErrorKind,
}

impl RunError {
    const fn new(kind: RunErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> RunErrorKind {
        self.kind
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "run operation failed ({:?})", self.kind)
    }
}

impl Error for RunError {}

/// A portable, non-path run identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RunId(String);

impl RunId {
    /// Validates a canonical run identifier that cannot encode a filesystem path.
    pub fn new(value: &str) -> Result<Self, RunError> {
        if is_portable_identifier(value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(RunError::new(RunErrorKind::InvalidRunId))
        }
    }

    /// Returns the canonical serialized identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Versioned bounded-retry policy: retry counts are policy data, not constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    version: &'static str,
    automatic_retry_budget: u32,
}

impl RetryPolicy {
    /// Returns the initial versioned bounded-retry policy.
    pub const fn v1() -> Self {
        Self {
            version: "project-run-retry-1",
            automatic_retry_budget: 2,
        }
    }

    /// Returns the versioned policy identifier folded into audit records.
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// Returns the number of automatic retries permitted after the first attempt.
    pub const fn automatic_retry_budget(&self) -> u32 {
        self.automatic_retry_budget
    }

    const fn max_attempts(&self) -> u32 {
        self.automatic_retry_budget + 1
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StageState {
    stage: Stage,
    status: StageStatus,
    attempts: u32,
    reason: Option<String>,
    result_snapshot: Option<ContentHash>,
}

impl StageState {
    const fn pending(stage: Stage) -> Self {
        Self {
            stage,
            status: StageStatus::Pending,
            attempts: 0,
            reason: None,
            result_snapshot: None,
        }
    }

    fn reset(&mut self) {
        self.status = StageStatus::Pending;
        self.attempts = 0;
        self.reason = None;
        self.result_snapshot = None;
    }

    fn to_value(&self) -> Value {
        json!({
            "stage": self.stage.code(),
            "status": self.status.code(),
            "attempts": self.attempts,
            "automatic_retries_exhausted": self.status == StageStatus::Unavailable,
            "reason": self.reason,
            "result_snapshot": self.result_snapshot.as_ref().map(ContentHash::as_str),
        })
    }
}

/// One project analysis run modeled as a preserving stage state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRun {
    run_id: RunId,
    lifecycle: RunLifecycle,
    policy: RetryPolicy,
    stages: Vec<StageState>,
    audit: Vec<AdminAuditEvent>,
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
        let all_complete = self
            .stages
            .iter()
            .all(|state| state.status == StageStatus::Complete);
        if all_complete {
            return StageStatus::Complete;
        }
        let any_usable = self
            .stages
            .iter()
            .any(|state| matches!(state.status, StageStatus::Complete | StageStatus::Partial));
        if any_usable {
            return StageStatus::Partial;
        }
        if self
            .stages
            .iter()
            .any(|state| state.status == StageStatus::Unavailable)
        {
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
    pub fn to_machine_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "run_state_version": RUN_STATE_VERSION,
            "run_id": self.run_id.as_str(),
            "lifecycle": self.lifecycle.code(),
            "status": self.status().code(),
            "ordinary_user_retry_available": self.ordinary_user_retry_available(),
            "retry_policy": {
                "version": self.policy.version,
                "automatic_retry_budget": self.policy.automatic_retry_budget,
            },
            "stages": self.stages.iter().map(StageState::to_value).collect::<Vec<_>>(),
            "audit_events": self.audit.iter().map(AdminAuditEvent::to_value).collect::<Vec<_>>(),
        })
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

fn validate_reason(reason: &str) -> Result<String, RunError> {
    if is_machine_code(reason) {
        Ok(reason.to_owned())
    } else {
        Err(RunError::new(RunErrorKind::InvalidReason))
    }
}

fn validate_timestamp(at: &str) -> Result<String, RunError> {
    if at.is_empty() || at.len() > MAX_TIMESTAMP_BYTES || at.chars().any(char::is_control) {
        Err(RunError::new(RunErrorKind::InvalidTimestamp))
    } else {
        Ok(at.to_owned())
    }
}

fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = false;
    for &byte in bytes {
        if matches!(byte, b'.' | b'_' | b'-') {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else {
            return false;
        }
    }
    !previous_separator
}

fn is_portable_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || value.contains("..") {
        return false;
    }
    let boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    boundary(bytes[0])
        && boundary(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|&byte| boundary(byte) || matches!(byte, b'.' | b'_' | b'-'))
}
