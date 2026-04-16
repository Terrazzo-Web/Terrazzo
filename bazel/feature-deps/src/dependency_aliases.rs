use std::collections::HashMap;

use crate::args::Args;

impl Args {
    /// Parses repeated `DEPENDENCY=LABEL` CLI arguments into a dependency-to-label map.
    ///
    /// Returns an error when any alias is missing the `=` separator or either side is empty.
    pub fn parse_dependency_aliases(&self) -> Result<HashMap<String, String>, String> {
        let mut dependency_aliases = HashMap::new();
        let raw_aliases = self.dependency_aliases;
        for dependency_alias in dependency_aliases {
            let Some((dependency, label)) = dependency_alias.split_once('=') else {
                return Err(format!(
                    "invalid dependency alias {dependency_alias:?}, expected DEPENDENCY=LABEL"
                ));
            };
            if dependency.is_empty() || label.is_empty() {
                return Err(format!(
                    "invalid dependency alias {dependency_alias:?}, expected DEPENDENCY=LABEL"
                ));
            }
            dependency_aliases.insert(dependency.to_owned(), label.to_owned());
        }
        Ok(dependency_aliases)
    }
}
