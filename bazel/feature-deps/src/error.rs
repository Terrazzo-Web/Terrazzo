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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::FeatureDepsError;
    use crate::args::FeatureAliasesError;
    use crate::args::ParseFeaturesError;

    #[test]
    fn renders_parse_features_error() {
        let error = FeatureDepsError::from(ParseFeaturesError::ManifestNotFound {
            path: PathBuf::from("/definitely/missing/Cargo.toml"),
            error: std::io::Error::from_raw_os_error(2),
        });
        assert_eq!(
            error.to_string(),
            r#"[ParseFeaturesError] [ManifestNotFound] Failed to read Cargo.toml manifest path="/definitely/missing/Cargo.toml" error=No such file or directory (os error 2)"#
        );
    }

    #[test]
    fn renders_feature_aliases_error() {
        let error = FeatureDepsError::from(FeatureAliasesError("bad".to_owned()));
        assert_eq!(
            error.to_string(),
            r#"[FeatureAliasesError] [FeatureAliasesError] Invalid dependency alias "bad", expected DEPENDENCY=LABEL"#
        );
    }

    #[test]
    fn renders_other_error() {
        let error = FeatureDepsError::from("boom".to_owned());
        assert_eq!(error.to_string(), "[Other] Other: boom");
    }
}
