use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use heck::ToShoutySnakeCase;
use toml::Table;

#[derive(Parser)]
struct Args {
    cargo_toml: PathBuf,
    output_bzl: PathBuf,
    #[arg(long = "dependency-alias")]
    dependency_aliases: Vec<String>,
    #[arg(long = "dependency-exclusion")]
    dependency_exclusion: Vec<String>,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let manifest = fs::read_to_string(&args.cargo_toml)
        .map_err(|error| format!("failed to read {}: {error}", args.cargo_toml.display()))?;
    let features = parse_features(&manifest)?;
    let dependency_aliases = parse_dependency_aliases(args.dependency_aliases)?;
    let dependency_exclusion = args
        .dependency_exclusion
        .into_iter()
        .collect::<HashSet<_>>();
    let output = render_bzl(&features, &dependency_aliases, &dependency_exclusion)?;
    fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}

fn parse_dependency_aliases(raw_aliases: Vec<String>) -> Result<HashMap<String, String>, String> {
    let mut dependency_aliases = HashMap::new();
    for raw_alias in raw_aliases {
        let Some((dependency, label)) = raw_alias.split_once('=') else {
            return Err(format!(
                "invalid dependency alias {raw_alias:?}, expected DEPENDENCY=LABEL"
            ));
        };
        if dependency.is_empty() || label.is_empty() {
            return Err(format!(
                "invalid dependency alias {raw_alias:?}, expected DEPENDENCY=LABEL"
            ));
        }
        dependency_aliases.insert(dependency.to_owned(), label.to_owned());
    }
    Ok(dependency_aliases)
}

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

    let expression = render_expression(&child_features, &dependencies);
    output.push_str(&format!(
        "{} = {}\n",
        feature_constant_name(feature_name),
        expression
    ));
    emitted.insert(feature_name.to_owned());

    Ok(())
}

fn render_expression(child_features: &BTreeSet<String>, dependencies: &BTreeSet<String>) -> String {
    let mut parts = child_features
        .iter()
        .map(|feature| feature_constant_name(feature))
        .collect::<Vec<_>>();

    if !dependencies.is_empty() || parts.is_empty() {
        let deps = dependencies
            .iter()
            .map(|dependency| format!("{dependency:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        parts.push(format!("[{deps}]"));
    }

    parts.join(" + ")
}

fn feature_constant_name(feature_name: &str) -> String {
    format!("{}_DEPS", feature_name.to_shouty_snake_case())
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
TERMINAL_DEPS = CLIENT_DEPS + [\"@crates//:scopeguard\"]\n"
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
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nDEBUG_DEPS = []\n"
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
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nSERVER_DEPS = []\n"
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
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nSERVER_DEPS = [\"//pty\",\n\"//remote/client\"]\n"
        );
    }
}
