mod analyze;
mod capabilities;
mod evaluation;
mod history;
mod serve;

use std::io::Write;

use crate::cli::{Cli, Command, HistoryCommand, ProjectCommand, ProjectSubcommand};
use crate::errors::{Outcome, RunError, emit};
use crate::output::json_bytes;
use crate::schema::validate;

#[cfg(test)]
pub(crate) use capabilities::ai_evaluation_capability;

pub(crate) fn run(cli: Cli, stderr: &mut dyn Write) -> Result<Outcome, RunError> {
    match cli.command {
        Command::Capabilities(arguments) => {
            let value = capabilities::capabilities();
            validate("capabilities", &value)?;
            Ok(emit(json_bytes(&value)?, arguments.output))
        }
        Command::Project(ProjectCommand {
            command: ProjectSubcommand::Analyze(arguments),
        }) => analyze::analyze(arguments),
        Command::Serve(arguments) => serve::serve(arguments, stderr),
        Command::History(HistoryCommand { command }) => history::history(command),
    }
}
