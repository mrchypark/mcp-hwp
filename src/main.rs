mod adapters;

use adapters::cli::{Cli, Commands};
use anyhow::{Result, bail};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                adapters::mcp_stdio::run_stdio_server()
            } else {
                bail!("only --stdio transport is supported")
            }
        }
        command => match adapters::cli::run(command) {
            Ok(()) => Ok(()),
            Err(err) => {
                eprintln!("{}", err.message);
                std::process::exit(1);
            }
        },
    }
}
