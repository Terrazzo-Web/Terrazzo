use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    pub cargo_toml: PathBuf,
    pub output_bzl: PathBuf,

    #[arg(long = "dependency-alias")]
    pub dependency_aliases: Vec<String>,

    #[arg(long = "dependency-exclusion")]
    pub dependency_exclusion: Vec<String>,
}
