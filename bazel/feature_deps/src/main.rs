use std::path::PathBuf;

use clap::Parser;

mod args;
mod error;
mod manager;
mod srcs;

use args::Args;
use error::FeatureDepsError;
use manager::Manager;

fn main() -> Result<(), FeatureDepsError> {
    let args = Args::parse();
    let features = args.parse_features()?;
    let dependency_aliases = args.parse_dependency_aliases()?;
    let dependency_exclusion = args.dependency_exclusion();
    let output = Manager::new(
        args.root_rs.into(),
        args.all_srcs.split(',').map(PathBuf::from).collect(),
        features,
        dependency_aliases,
        dependency_exclusion,
    )
    .render_bzl()?;
    std::fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}
