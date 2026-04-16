use std::path::PathBuf;

use nameth::NamedEnumValues as _;
use nameth::nameth;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum FeatureDepsError {
    #[error("[{n}] Failed to read Cargo.toml manifest path={path:?} error={error}", n = self.name())]
    ManifestNotFound {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("[FD001:{n}] Failed to parse Cargo.toml: {error}", n = self.name())]
    ManifestMalformed { error: toml::de::Error },

    #[error("[FD002:{n}] [features] must be a TOML table", n = self.name())]
    FeaturesTableInvalid,

    #[error("[FD003:{n}] feature {feature_name:?} must be an array", n = self.name())]
    FeatureEntriesInvalid { feature_name: String },

    #[error("[FD004:{n}] feature {feature_name:?} must only contain strings", n = self.name())]
    FeatureEntryInvalid { feature_name: String },

    #[error("[{n}] Invalid dependency alias {0:?}, expected DEPENDENCY=LABEL", n = self.name())]
    DependencyAliasInvalid(String),

    #[error("[{n}] Other: {0}", n = self.name())]
    Other(String),
}

impl From<String> for FeatureDepsError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}
