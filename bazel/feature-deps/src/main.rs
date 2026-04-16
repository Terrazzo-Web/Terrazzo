use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use clap::Parser;
use heck::ToShoutySnakeCase;

mod args;
mod error;

use args::Args;
use error::FeatureDepsError;

fn main() -> Result<(), FeatureDepsError> {
    let args = Args::parse();
    let features = args.parse_features()?;
    let dependency_aliases = args.parse_dependency_aliases()?;
    let dependency_exclusion = args.dependency_exclusion();
    let output = render_bzl(&features, &dependency_aliases, &dependency_exclusion)?;
    std::fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}

/// Renders the complete `.bzl` output for all features in dependency order.
///
/// Features are emitted once, sorted by name for stable output.
fn render_bzl(
    features: &HashMap<String, Vec<String>>,
    dependency_aliases: &HashMap<String, String>,
    dependency_exclusion: &HashSet<String>,
) -> Result<String, String> {
    let mut output = String::from("\"\"\"Generated feature dependency constants.\"\"\"\n\n");
    let mut emitted = HashSet::new();

    let mut feature_names = features.keys().cloned().collect::<Vec<_>>();
    feature_names.sort();

    for feature_name in feature_names {
        emit_feature(
            &mut output,
            &mut emitted,
            features,
            dependency_aliases,
            dependency_exclusion,
            &feature_name,
        )?;
    }

    Ok(output)
}

/// Emits the `*_DEPS` and `*_FEATURES` constants for one feature and any nested child features.
///
/// Dependency entries are converted into Bazel labels, excluded dependencies are skipped, and
/// slash-qualified feature references are ignored because they refer to other crates.
fn emit_feature(
    output: &mut String,
    emitted: &mut HashSet<String>,
    features: &HashMap<String, Vec<String>>,
    dependency_aliases: &HashMap<String, String>,
    dependency_exclusion: &HashSet<String>,
    feature_name: &str,
) -> Result<(), String> {
    if emitted.contains(feature_name) {
        return Ok(());
    }

    let entries = features
        .get(feature_name)
        .ok_or_else(|| format!("feature {feature_name:?} is not defined"))?;

    let mut child_features = BTreeSet::new();
    let mut dependencies = BTreeSet::new();

    for entry in entries {
        if let Some(dependency) = entry.strip_prefix("dep:") {
            if !dependency_exclusion.contains(dependency) {
                dependencies.insert(format_dependency_label(dependency, dependency_aliases));
            }
            continue;
        }
        if entry.contains('/') {
            continue;
        }

        emit_feature(
            output,
            emitted,
            features,
            dependency_aliases,
            dependency_exclusion,
            entry,
        )?;
        child_features.insert(entry.clone());
    }

    let deps_expression = render_expression(&child_features, &dependencies, "DEPS", false);
    let features_expression = render_expression(
        &child_features,
        &BTreeSet::from([feature_name.to_owned()]),
        "FEATURES",
        true,
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

fn render_expression(
    child_features: &BTreeSet<String>,
    values: &BTreeSet<String>,
    suffix: &str,
    values_first: bool,
) -> String {
    let child_parts = child_features
        .iter()
        .map(|feature| feature_constant_name(feature, suffix))
        .collect::<Vec<_>>();
    let mut parts = Vec::new();

    if !values.is_empty() || child_parts.is_empty() {
        let values = values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        if values_first {
            parts.push(format!("[{values}]"));
        }
        parts.extend(child_parts.iter().cloned());
        if !values_first {
            parts.push(format!("[{values}]"));
        }
    } else {
        parts.extend(child_parts);
    }

    parts.join(" + ")
}

fn feature_constant_name(feature_name: &str, suffix: &str) -> String {
    format!("{}_{}", feature_name.to_shouty_snake_case(), suffix)
}

fn format_dependency_label(
    dependency: &str,
    dependency_aliases: &HashMap<String, String>,
) -> String {
    dependency_aliases
        .get(dependency)
        .cloned()
        .unwrap_or_else(|| format!("@crates//:{dependency}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::render_bzl;
    use crate::args::Args;

    fn parse_features(
        manifest: &str,
    ) -> Result<HashMap<String, Vec<String>>, crate::args::ParseFeaturesError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let cargo_toml = std::env::temp_dir().join(format!("feature-deps-{unique}.toml"));
        std::fs::write(&cargo_toml, manifest).unwrap();

        let args = Args::parse_from(["feature-deps", cargo_toml.to_str().unwrap(), "out.bzl"]);
        let result = args.parse_features();
        let _ = std::fs::remove_file(cargo_toml);
        result
    }

    #[test]
    fn generates_dependencies_after_children() {
        let features = parse_features(
            r#"
[features]
client = ["dep:stylance"]
terminal = ["client", "dep:scopeguard", "web-sys/Window"]
"#,
        )
        .unwrap();

        let output = render_bzl(&features, &HashMap::new(), &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\
\"\"\"Generated feature dependency constants.\"\"\"\n\
\n\
CLIENT_DEPS = [\"@crates//:stylance\"]\n\
CLIENT_FEATURES = [\"client\"]\n\
TERMINAL_DEPS = CLIENT_DEPS + [\"@crates//:scopeguard\"]\n\
TERMINAL_FEATURES = [\"terminal\"] + CLIENT_FEATURES\n"
        );
    }

    #[test]
    fn generates_empty_lists_for_leaf_features() {
        let features = parse_features(
            r#"
[features]
debug = []
"#,
        )
        .unwrap();

        let output = render_bzl(&features, &HashMap::new(), &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nDEBUG_DEPS = []\nDEBUG_FEATURES = [\"debug\"]\n"
        );
    }

    #[test]
    fn fails_for_unknown_child_features() {
        let features = parse_features(
            r#"
[features]
terminal = ["missing"]
"#,
        )
        .unwrap();

        let error = render_bzl(&features, &HashMap::new(), &HashSet::new()).unwrap_err();

        assert_eq!(error, "feature \"missing\" is not defined");
    }

    #[test]
    fn excludes_selected_dependencies() {
        let features = parse_features(
            r#"
[features]
server = ["dep:terrazzo-pty", "dep:trz-gateway-client"]
"#,
        )
        .unwrap();

        let excluded = HashSet::from(["terrazzo-pty".to_owned(), "trz-gateway-client".to_owned()]);
        let output = render_bzl(&features, &HashMap::new(), &excluded).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nSERVER_DEPS = []\nSERVER_FEATURES = [\"server\"]\n"
        );
    }

    #[test]
    fn applies_dependency_aliases() {
        let features = parse_features(
            r#"
[features]
server = ["dep:terrazzo-pty", "dep:trz-gateway-client"]
"#,
        )
        .unwrap();

        let dependency_aliases = HashMap::from([
            ("terrazzo-pty".to_owned(), "//pty".to_owned()),
            (
                "trz-gateway-client".to_owned(),
                "//remote/client".to_owned(),
            ),
        ]);
        let output = render_bzl(&features, &dependency_aliases, &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nSERVER_DEPS = [\"//pty\",\n\"//remote/client\"]\nSERVER_FEATURES = [\"server\"]\n"
        );
    }
}
