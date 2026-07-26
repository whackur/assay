//! Project-intelligence run orchestration and administrator recovery.
//! Models the analysis pipeline as a stage state machine. A partial stage failure never fails the whole run: completed stages keep their immutable snapshot; failed stages carry `partial`/`unavailable` plus a redacted reason. Bounded versioned retries; once spent, only an administrator capability may rerun, soft-delete, restore, or purge, each appending a secret-free audit event.
//! The four-state vocabulary (`pending`, `complete`, `partial`, `unavailable`) mirrors domain availability without importing it: a stage status is a pipeline position, never disguised as zero or success. No I/O — timestamps and identifiers are injected.

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
