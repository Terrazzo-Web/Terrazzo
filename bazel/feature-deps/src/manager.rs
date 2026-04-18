use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use heck::ToShoutySnakeCase as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;

use crate::srcs::CollectSrcsError;
use crate::srcs::SrcsManager;

pub struct Manager {
    features: HashMap<String, Vec<String>>,
    dependency_aliases: HashMap<String, String>,
    dependency_exclusion: HashSet<String>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RenderBzlError {
    #[error("[{n}] Feature {feature_name:?} is not defined", n = self.name())]
    FeatureNotFound { feature_name: String },

    #[error("[{n}] {0}", n = self.name())]
    CollectSrcsError(#[from] CollectSrcsError),
}

impl Manager {
    pub fn new(
        features: HashMap<String, Vec<String>>,
        dependency_aliases: HashMap<String, String>,
        dependency_exclusion: HashSet<String>,
    ) -> Self {
        Self {
            features,
            dependency_aliases,
            dependency_exclusion,
        }
    }

    /// Renders the complete `.bzl` output for all features in dependency order.
    ///
    /// Features are emitted once, sorted by name for stable output.
    pub fn render_bzl(&self, root_rs: Rc<Path>) -> Result<String, RenderBzlError> {
        let mut output = String::from(r#""""Generated feature dependency constants.""""#);
        output.push_str("\n");
        output.push_str("\n");
        output.push_str(r#"load("//bazel/feature-deps:defs.bzl", "base_compute_srcs")"#);
        output.push_str("\n");
        {
            let output = &mut output;

            let mut feature_names = self.features.keys().cloned().collect::<Vec<_>>();
            feature_names.sort();

            self.emit_all_features(output, &feature_names);
            let mut emitted = HashSet::new();
            for feature_name in &feature_names {
                self.emit_feature(output, &mut emitted, feature_name)?;
            }

            let mut srcs_manager = SrcsManager::new(&feature_names, root_rs);
            srcs_manager.emit_all_excluded_srcs(output)?;
            srcs_manager.emit_all_srcs(output);
            output.push_str(
                r#"
def compute_srcs(features):
    return base_compute_srcs(features, _ALL_SRCS, _ALL_FEATURES, _EXCLUSION_MAP)
            "#,
            );
        }
        Ok(output)
    }

    /// Emits the `*_DEPS` and `*_FEATURES` constants for one feature and any nested child features.
    ///
    /// Dependency entries are converted into Bazel labels, excluded dependencies are skipped, and
    /// slash-qualified feature references are ignored because they refer to other crates.
    fn emit_feature(
        &self,
        output: &mut String,
        emitted: &mut HashSet<String>,
        feature_name: &str,
    ) -> Result<(), RenderBzlError> {
        if emitted.contains(feature_name) {
            return Ok(());
        }

        let entries =
            self.features
                .get(feature_name)
                .ok_or_else(|| RenderBzlError::FeatureNotFound {
                    feature_name: feature_name.to_owned(),
                })?;

        let mut child_features = BTreeSet::new();
        let mut dependencies = BTreeSet::new();

        for entry in entries {
            if let Some(dependency) = entry.strip_prefix("dep:") {
                if !self.dependency_exclusion.contains(dependency) {
                    dependencies.insert(self.format_dependency_label(dependency));
                }
                continue;
            }
            if entry.contains('/') {
                continue;
            }

            self.emit_feature(output, emitted, entry)?;
            child_features.insert(entry.clone());
        }

        let deps_expression = render_expression(&child_features, &dependencies, "DEPS");
        let features_expression = render_expression(
            &child_features,
            &BTreeSet::from([feature_name.to_owned()]),
            "FEATURES",
        );
        output.push_str(&format!(
            "{} = {}\n",
            feature_constant_name(feature_name, "DEPS"),
            deps_expression
        ));
        output.push_str(&format!(
            "{} = {}\n",
            feature_constant_name(feature_name, "FEATURES"),
            features_expression
        ));
        emitted.insert(feature_name.to_owned());

        Ok(())
    }

    fn format_dependency_label(&self, dependency: &str) -> String {
        self.dependency_aliases
            .get(dependency)
            .cloned()
            .unwrap_or_else(|| format!("@crates//:{dependency}"))
    }

    fn emit_all_features(&self, output: &mut String, feature_names: &[String]) {
        output.push_str(&format!(
            "_ALL_FEATURES = [{}]\n",
            feature_names
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
}

fn render_expression(
    child_features: &BTreeSet<String>,
    values: &BTreeSet<String>,
    suffix: &str,
) -> String {
    let child_parts = child_features
        .iter()
        .map(|feature| feature_constant_name(feature, suffix))
        .collect::<Vec<_>>();
    let mut parts = vec![];

    if !values.is_empty() || child_parts.is_empty() {
        let values = values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        parts.extend(child_parts.iter().cloned());
        parts.push(format!("[{values}]"));
    } else {
        parts.extend(child_parts);
    }

    parts.join(" + ")
}

fn feature_constant_name(feature_name: &str, suffix: &str) -> String {
    format!("{}_{}", feature_name.to_shouty_snake_case(), suffix)
}
