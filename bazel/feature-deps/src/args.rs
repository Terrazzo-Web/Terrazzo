use std::path::PathBuf;

use crate::error::FeatureDepsError;
use clap::Parser;
use std::collections::HashMap;
use toml::Table;

#[derive(Parser)]
pub struct Args {
    pub cargo_toml: PathBuf,
    pub output_bzl: PathBuf,

    #[arg(long = "dependency-alias")]
    pub dependency_aliases: Vec<String>,

    #[arg(long = "dependency-exclusion")]
    pub dependency_exclusion: Vec<String>,
}

impl Args {
    /// Extracts the `[features]` table from a Cargo manifest and normalizes it into owned strings.
    ///
    /// Returns an empty map when the manifest has no `[features]` section.
    pub fn parse_features(&self) -> Result<HashMap<String, Vec<String>>, FeatureDepsError> {
        let value: Table = self
            .cargo_toml
            .parse()
            .map_err(FeatureDepsError::ManifestMalformed)?;
        let Some(features) = value.get("features") else {
            return Ok(HashMap::new());
        };
        let Some(table) = features.as_table() else {
            return Err("[features] must be a TOML table".to_owned());
        };

        let mut result = HashMap::new();
        for (name, value) in table {
            let Some(items) = value.as_array() else {
                return Err(format!("feature {name:?} must be an array"));
            };
            let mut entries = Vec::with_capacity(items.len());
            for item in items {
                let Some(item) = item.as_str() else {
                    return Err(format!("feature {name:?} must only contain strings"));
                };
                entries.push(item.to_owned());
            }
            result.insert(name.clone(), entries);
        }
        Ok(result)
    }

    /// Parses repeated `DEPENDENCY=LABEL` CLI arguments into a dependency-to-label map.
    ///
    /// Returns an error when any alias is missing the `=` separator or either side is empty.
    pub fn parse_dependency_aliases(&self) -> Result<HashMap<String, String>, FeatureDepsError> {
        let mut result = HashMap::new();
        for dependency_alias in &self.dependency_aliases {
            let Some((dependency, label)) = dependency_alias.split_once('=') else {
                return Err(FeatureDepsError::DependencyAliasInvalid(
                    dependency_alias.clone(),
                ));
            };
            if dependency.is_empty() || label.is_empty() {
                return Err(FeatureDepsError::DependencyAliasInvalid(
                    dependency_alias.clone(),
                ));
            }
            result.insert(dependency.to_owned(), label.to_owned());
        }
        Ok(result)
    }
}
