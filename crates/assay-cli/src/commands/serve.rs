use std::io::Write;

use assay_local::{LocalHistoryStore, LoopbackListener, run as serve_run, serve_next};

use crate::cli::ServeArgs;
use crate::errors::{Outcome, RunError, history_write_error, serve_bind_error, serve_failed_error};

pub(crate) fn serve(arguments: ServeArgs, stderr: &mut dyn Write) -> Result<Outcome, RunError> {
    let _no_color = arguments.no_color;
    let store = LocalHistoryStore::open(&arguments.history).map_err(|_| history_write_error())?;
    let listener = LoopbackListener::bind(arguments.port).map_err(|_| serve_bind_error())?;
    if let Ok(address) = listener.local_addr() {
        let _ = writeln!(stderr, "listening on http://{address}");
    }
    let served = if arguments.once {
        serve_next(&listener, &store)
    } else {
        serve_run(&listener, &store)
    };
    served.map_err(|_| serve_failed_error())?;
    Ok(Outcome::Served)
}
