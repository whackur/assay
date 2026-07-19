use serde_json::{Value, json};

use crate::run::id::RunId;
use crate::run::lifecycle::{AdminAuditEvent, RunLifecycle};
use crate::run::policy::RetryPolicy;
use crate::run::stage::StageStatus;
use crate::run::state::StageState;

pub(crate) const RUN_STATE_VERSION: &str = "project-run-1";
pub(crate) const SCHEMA_VERSION: &str = "1.0.0";

pub(crate) fn run_machine_value(
    run_id: &RunId,
    lifecycle: RunLifecycle,
    status: StageStatus,
    ordinary_user_retry_available: bool,
    policy: RetryPolicy,
    stages: &[StageState],
    audit: &[AdminAuditEvent],
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "run_state_version": RUN_STATE_VERSION,
        "run_id": run_id.as_str(),
        "lifecycle": lifecycle.code(),
        "status": status.code(),
        "ordinary_user_retry_available": ordinary_user_retry_available,
        "retry_policy": {
            "version": policy.version,
            "automatic_retry_budget": policy.automatic_retry_budget,
        },
        "stages": stages.iter().map(StageState::to_value).collect::<Vec<_>>(),
        "audit_events": audit.iter().map(AdminAuditEvent::to_value).collect::<Vec<_>>(),
    })
}
