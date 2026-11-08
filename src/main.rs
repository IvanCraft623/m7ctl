use clap::Parser;
use m7ctl::cli::{self, Cli, Context};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = Context::system(cli.device.clone()).and_then(|ctx| cli::run(cli.command, &ctx, &mut std::io::stdout()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("m7ctl: {e}");
            ExitCode::FAILURE
        }
    }
}
