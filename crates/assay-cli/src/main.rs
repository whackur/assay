use std::io;

use assay_cli::{Cli, execute};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    let exit = execute(cli, &mut io::stdout().lock(), &mut io::stderr().lock());
    std::process::exit(exit);
}
