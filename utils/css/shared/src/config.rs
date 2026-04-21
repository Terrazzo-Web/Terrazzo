use std::path::Path;
use std::path::PathBuf;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub output_file: PathBuf,
    pub extensions: Vec<String>,
    pub folders: Vec<PathBuf>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("[{n}] Failed to load manifest at '{0}': {1}", n = self.name())]
    LoadManifestError(PathBuf, std::io::Error),

    #[error("[{n}] Failed to parse manifest: {0}", n = self.name())]
    ParseManifestError(toml::de::Error),
}

impl Config {
    pub fn load(manifest_dir: &Path) -> Result<Self, ConfigError> {
        let manifest_path = manifest_dir.join("Cargo.toml");
        let cargo_toml_contents = std::fs::read_to_string(&manifest_path)
            .map_err(|error| ConfigError::LoadManifestError(manifest_path, error))?;
        let cargo_toml: CargoToml =
            toml::from_str(&cargo_toml_contents).map_err(ConfigError::ParseManifestError)?;
        let mut config = cargo_toml.package.metadata.css;
        for folder in &mut config.folders {
            *folder = manifest_dir.join(&folder);
        }
        config.output_file = manifest_dir.join(&config.output_file);
        Ok(config)
    }
}

#[derive(Deserialize)]
struct CargoToml {
    package: CargoPackage,
}

#[derive(Deserialize)]
struct CargoPackage {
    metadata: CargoMetadata,
}

#[derive(Deserialize)]
struct CargoMetadata {
    css: Config,
}
