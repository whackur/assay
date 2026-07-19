use std::{error::Error, fmt};

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
    pub(crate) const fn new(kind: RunErrorKind) -> Self {
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
