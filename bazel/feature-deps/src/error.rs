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

    #[error("[{n}] Invalid dependency alias {0:?}, expected DEPENDENCY=LABEL", n = self.name())]
    DependencyAliasInvalid(String),

    #[error("[{n}] Other: {0}", n = self.name())]
    Other(#[from] String),
}
