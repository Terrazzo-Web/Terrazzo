use std::path::PathBuf;

#[nameth]
#[derive(thiserror::Error, Debug)]
enum FeatureDepsError {
    #[error("[{n}] Failed to read Cargo.toml manifest path={0:?} error={error}", n = self.name())]
    ManifestNotFound {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("[{n}] Invalid dependency alias {0:?}, expected DEPENDENCY=LABEL", n = self.name())]
    DependencyAliasInvalid(dependency_alias),

    #[error("[{n}] Other: {0}", n = self.name())]
    Other(#[from] String),
}
