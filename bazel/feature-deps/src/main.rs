use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use clap::Parser;
use heck::ToShoutySnakeCase;
use toml::Table;

mod args;
mod dependency_aliases;

use args::Args;

fn main() -> Result<(), String> {
    let args = Args::parse();
    let manifest = std::fs::read_to_string(&args.cargo_toml)
        .map_err(|error| format!("failed to read {}: {error}", args.cargo_toml.display()))?;
    let features = parse_features(&manifest)?;
    let dependency_aliases = args.parse_dependency_aliases()?;
    let dependency_exclusion = args
        .dependency_exclusion
        .into_iter()
        .collect::<HashSet<_>>();
    let output = render_bzl(&features, &dependency_aliases, &dependency_exclusion)?;
    std::fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}

/// Extracts the `[features]` table from a Cargo manifest and normalizes it into owned strings.
///
/// Returns an empty map when the manifest has no `[features]` section.
fn parse_features(manifest: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let value: Table = manifest
        .parse()
        .map_err(|error| format!("failed to parse Cargo.toml: {error}"))?;
    let Some(features) = value.get("features") else {
        return Ok(HashMap::new());
    };
    let Some(table) = features.as_table() else {
        return Err("[features] must be a TOML table".to_owned());
    };

    let mut result = HashMap::new();
    for (name, value) in table {
        let Some(items) = value.as_array() else {
            return Err(format!("feature {name:?} must be an array"));
        };
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let Some(item) = item.as_str() else {
                return Err(format!("feature {name:?} must only contain strings"));
            };
            entries.push(item.to_owned());
        }
        result.insert(name.clone(), entries);
    }
    Ok(result)
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

    use super::parse_features;
    use super::render_bzl;

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
