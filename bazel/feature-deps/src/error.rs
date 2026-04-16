use nameth::nameth;
use nameth::NamedEnumValues as _;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum FeatureDepsError {
    #[error("[{n}] {0}", n = self.name())]
    ParseFeaturesError(#[from] crate::args::ParseFeaturesError),

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
