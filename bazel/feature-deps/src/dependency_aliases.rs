use std::collections::HashMap;

use crate::args::Args;

impl Args {
    /// Parses repeated `DEPENDENCY=LABEL` CLI arguments into a dependency-to-label map.
    ///
    /// Returns an error when any alias is missing the `=` separator or either side is empty.
    pub fn parse_dependency_aliases(&self) -> Result<HashMap<String, String>, FeatureDepsError> {
        let mut result = HashMap::new();
        for dependency_alias in self.dependency_aliases {
            let Some((dependency, label)) = dependency_alias.split_once('=') else {
                return Err(FeatureDepsError::DependencyAliasInvalid(dependency_alias));
            };
            if dependency.is_empty() || label.is_empty() {
                return Err(FeatureDepsError::DependencyAliasInvalid(dependency_alias));
            }
            result.insert(dependency.to_owned(), label.to_owned());
        }
        Ok(result)
    }
}
