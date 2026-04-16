use nameth::NamedEnumValues as _;
use nameth::nameth;

use crate::args::FeatureAliasesError;
use crate::args::ParseFeaturesError;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum FeatureDepsError {
    #[error("[{n}] {0}", n = self.name())]
    ParseFeaturesError(#[from] ParseFeaturesError),

    #[error("[{n}] {0}", n = self.name())]
    FeatureAliasesError(#[from] FeatureAliasesError),

    #[error("[{n}] Other: {0}", n = self.name())]
    Other(String),
}

impl From<String> for FeatureDepsError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}
