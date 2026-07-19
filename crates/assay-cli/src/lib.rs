//! Thin command-line delivery boundary for Assay.

#![forbid(unsafe_code)]

pub mod evaluators;

mod cli;
mod commands;
mod errors;
mod git;
mod output;
mod schema;
mod time;

#[cfg(test)]
mod tests;

use std::io::Write;

pub use cli::Cli;

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// Executes parsed CLI delivery with explicit output streams.
pub fn execute(cli: Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match commands::run(cli, stderr) {
        Ok(errors::Outcome::Emit { bytes, destination }) => {
            match output::write_output(&bytes, &destination, stdout) {
                Ok(()) => 0,
                Err(code) => {
                    let _ = writeln!(stderr, "error: output_failed code={code}");
                    12
                }
            }
        }
        Ok(errors::Outcome::Served) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "error: {}", error.message);
            error.exit_code
        }
    }
}
