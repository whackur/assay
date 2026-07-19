use assay_domain::ContentHash;
use serde_json::{Value, json};

use crate::run::stage::{Stage, StageStatus};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StageState {
    pub(crate) stage: Stage,
    pub(crate) status: StageStatus,
    pub(crate) attempts: u32,
    pub(crate) reason: Option<String>,
    pub(crate) result_snapshot: Option<ContentHash>,
}

impl StageState {
    pub(crate) const fn pending(stage: Stage) -> Self {
        Self {
            stage,
            status: StageStatus::Pending,
            attempts: 0,
            reason: None,
            result_snapshot: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.status = StageStatus::Pending;
        self.attempts = 0;
        self.reason = None;
        self.result_snapshot = None;
    }

    pub(crate) fn to_value(&self) -> Value {
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
