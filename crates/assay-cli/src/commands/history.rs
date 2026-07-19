use serde_json::json;

use assay_local::LocalAdministrator;
use assay_local::LocalHistoryStore;

use crate::cli::HistorySubcommand;
use crate::errors::{Outcome, RunError, emit, history_operation_error, history_write_error};
use crate::output::json_bytes;
use crate::time::current_time;

pub(crate) fn history(command: HistorySubcommand) -> Result<Outcome, RunError> {
    let (action, arguments) = match command {
        HistorySubcommand::Delete(arguments) => ("soft_delete", arguments),
        HistorySubcommand::Restore(arguments) => ("restore", arguments),
        HistorySubcommand::Purge(arguments) => ("purge", arguments),
    };
    let _no_color = arguments.no_color;
    let store = LocalHistoryStore::open(&arguments.history).map_err(|_| history_write_error())?;
    let operator = LocalAdministrator::assume_local_operator();
    let at = current_time()?;
    let result = match action {
        "soft_delete" => store.soft_delete(&arguments.id, &operator, &at),
        "restore" => store.restore(&arguments.id, &operator, &at),
        _ => store.purge(&arguments.id, &operator, &at),
    };
    result.map_err(|_| history_operation_error())?;
    let value = json!({
        "schema_version": "1.0.0",
        "action": action,
        "id": arguments.id,
        "status": "ok"
    });
    Ok(emit(json_bytes(&value)?, arguments.output))
}
