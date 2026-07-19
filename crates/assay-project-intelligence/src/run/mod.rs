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

mod error;
mod id;
mod lifecycle;
mod mapping;
mod policy;
mod project_run;
mod stage;
mod state;
mod validation;

pub use error::{RunError, RunErrorKind};
pub use id::RunId;
pub use lifecycle::{AdminAction, AdminAuditEvent, Administrator, RunLifecycle};
pub use policy::RetryPolicy;
pub use project_run::ProjectRun;
pub use stage::{AttemptDisposition, PIPELINE_STAGES, Stage, StageAttempt, StageStatus};
