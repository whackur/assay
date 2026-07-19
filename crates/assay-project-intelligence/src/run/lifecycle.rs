use serde_json::{Value, json};

use crate::run::id::RunId;
use crate::run::stage::Stage;

/// The record lifecycle of a whole run: soft deletion mirrors the local journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunLifecycle {
    Active,
    Deleted,
    Purged,
}

impl RunLifecycle {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
            Self::Purged => "purged",
        }
    }
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
    pub(crate) const fn code(self) -> &'static str {
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
    pub(crate) action: AdminAction,
    pub(crate) run_id: RunId,
    pub(crate) stage: Option<Stage>,
    pub(crate) policy_version: &'static str,
    pub(crate) recorded_at: String,
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

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "action": self.action.code(),
            "run_id": self.run_id.as_str(),
            "stage": self.stage.map(Stage::code),
            "policy_version": self.policy_version,
            "recorded_at": self.recorded_at,
        })
    }
}
