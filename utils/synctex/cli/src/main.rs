use std::process::ExitCode;

use clap as _;
#[cfg(feature = "bazel")]
use runfiles as _;
use terrazzo_synctex as _;
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
