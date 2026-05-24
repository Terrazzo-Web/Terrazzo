use std::process::ExitCode;

#[cfg(not(feature = "bazel"))]
use clap as _;
#[cfg(not(feature = "bazel"))]
use runfiles as _;
#[cfg(not(feature = "bazel"))]
use thiserror as _;

fn main() -> ExitCode {
    match terrazzo_synctex_cli::run(std::env::args_os(), std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
