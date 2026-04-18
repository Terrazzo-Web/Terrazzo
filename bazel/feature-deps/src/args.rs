use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use nameth::NamedEnumValues as _;
use nameth::nameth;

#[derive(Parser)]
pub struct Args {
    pub output_bzl: PathBuf,
    pub package_name: PathBuf,
    pub manifest: PathBuf,
    pub root_rs: PathBuf,

    #[arg(long = "dependency-alias")]
    dependency_aliases: Vec<String>,

    #[arg(long = "dependency-exclusion")]
    dependency_exclusion: Vec<String>,
}

impl Args {
    /// Extracts the `[features]` table from a Cargo manifest and normalizes it into owned strings.
    ///
    /// Returns an empty map when the manifest has no `[features]` section.
    pub fn parse_features(&self) -> Result<HashMap<String, Vec<String>>, ParseFeaturesError> {
        let manifest: toml::Table = std::fs::read_to_string(&self.manifest)
            .map_err(|error| ParseFeaturesError::ManifestNotFound {
                path: self.manifest.clone(),
                error,
            })?
            .parse()
            .map_err(ParseFeaturesError::ManifestMalformed)?;
        let Some(features) = manifest.get("features") else {
            return Ok(HashMap::new());
        };
        let Some(features) = features.as_table() else {
            return Err(ParseFeaturesError::FeaturesMalformed);
        };

        let mut result = HashMap::new();
        for (feature_name, feature_dependencies) in features {
            let feature_dependencies = feature_dependencies.as_array().ok_or_else(|| {
                ParseFeaturesError::FeatureMalformed {
                    feature_name: feature_name.clone(),
                }
            })?;
            let entries = feature_dependencies
                .iter()
                .map(|dependency| {
                    Ok(dependency
                        .as_str()
                        .ok_or_else(|| ParseFeaturesError::FeatureDependencyInvalid {
                            feature_name: feature_name.clone(),
                            dependency: dependency.clone(),
                        })?
                        .to_owned())
                })
                .collect::<Result<Vec<_>, ParseFeaturesError>>()?;
            result.insert(feature_name.clone(), entries);
        }
        Ok(result)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ParseFeaturesError {
    #[error("[{n}] Failed to read Cargo.toml manifest path={path:?} error={error}", n = self.name())]
    ManifestNotFound {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("[{n}] Failed to parse Cargo.toml: {0}", n = self.name())]
    ManifestMalformed(toml::de::Error),

    #[error("[{n}] '[features]' is not a TOML table", n = self.name())]
    FeaturesMalformed,

    #[error("[{n}] Feature {feature_name:?} is not a list of strings", n = self.name())]
    FeatureMalformed { feature_name: String },

    #[error("[{n}] Feature {feature_name:?} is not an array of strings: {dependency:?}", n = self.name())]
    FeatureDependencyInvalid {
        feature_name: String,
        dependency: toml::Value,
    },
}

impl Args {
    /// Parses repeated `DEPENDENCY=LABEL` CLI arguments into a dependency-to-label map.
    ///
    /// Returns an error when any alias is missing the `=` separator or either side is empty.
    pub fn parse_dependency_aliases(&self) -> Result<HashMap<String, String>, FeatureAliasesError> {
        let mut result = HashMap::new();
        for dependency_alias in &self.dependency_aliases {
            let (dependency, label) = dependency_alias
                .split_once('=')
                .ok_or_else(|| FeatureAliasesError(dependency_alias.clone()))?;
            if dependency.is_empty() || label.is_empty() {
                return Err(FeatureAliasesError(dependency_alias.clone()));
            }
            result.insert(dependency.to_owned(), label.to_owned());
        }
        Ok(result)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
#[error("[{FEATURE_ALIASES_ERROR}] Invalid dependency alias {0:?}, expected DEPENDENCY=LABEL")]
pub struct FeatureAliasesError(pub String);

impl Args {
    pub fn dependency_exclusion(&self) -> HashSet<String> {
        self.dependency_exclusion.iter().cloned().collect()
    }
}
