use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use heck::ToShoutySnakeCase;
use toml::Table;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManifestData {
    pub(crate) features: HashMap<String, Vec<String>>,
    pub(crate) lib_rs_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceLayout {
    pub(crate) module_root: PathBuf,
    pub(crate) lib_rs_path: PathBuf,
}

impl SourceLayout {
    pub(crate) fn new(cargo_toml_path: &Path, lib_rs_path: &Path) -> Result<Self, String> {
        let Some(crate_dir) = cargo_toml_path.parent() else {
            return Err(format!(
                "could not determine crate directory from {}",
                cargo_toml_path.display()
            ));
        };
        let lib_rs_path = crate_dir.join(lib_rs_path);
        let Some(module_root) = lib_rs_path.parent() else {
            return Err(format!(
                "could not determine module root from {}",
                lib_rs_path.display()
            ));
        };
        Ok(Self {
            module_root: module_root.to_path_buf(),
            lib_rs_path,
        })
    }
}

pub(crate) fn parse_dependency_aliases(
    raw_aliases: Vec<String>,
) -> Result<HashMap<String, String>, String> {
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

pub(crate) fn parse_manifest(manifest: &str) -> Result<ManifestData, String> {
    let value: Table = manifest
        .parse()
        .map_err(|error| format!("failed to parse Cargo.toml: {error}"))?;
    let features = parse_features_from_table(&value)?;
    let lib_rs_path = parse_lib_rs_path(&value)?;
    Ok(ManifestData {
        features,
        lib_rs_path,
    })
}

#[cfg(test)]
pub(crate) fn parse_features(manifest: &str) -> Result<HashMap<String, Vec<String>>, String> {
    Ok(parse_manifest(manifest)?.features)
}

fn parse_features_from_table(value: &Table) -> Result<HashMap<String, Vec<String>>, String> {
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

fn parse_lib_rs_path(value: &Table) -> Result<PathBuf, String> {
    let Some(lib) = value.get("lib") else {
        return Ok(PathBuf::from("src/lib.rs"));
    };
    let Some(lib_table) = lib.as_table() else {
        return Err("[lib] must be a TOML table".to_owned());
    };
    let Some(path) = lib_table.get("path") else {
        return Ok(PathBuf::from("src/lib.rs"));
    };
    let Some(path) = path.as_str() else {
        return Err("[lib].path must be a string".to_owned());
    };
    Ok(PathBuf::from(path))
}

pub(crate) fn collect_child_features(entries: &[String]) -> BTreeSet<String> {
    entries
        .iter()
        .filter(|entry| !entry.starts_with("dep:") && !entry.contains('/'))
        .cloned()
        .collect()
}

pub(crate) fn render_expression(
    child_features: &[String],
    values: &BTreeSet<String>,
    suffix: &str,
    values_first: bool,
) -> String {
    let mut parts = Vec::new();

    if !values.is_empty() || child_features.is_empty() {
        let values = values
            .iter()
            .map(|value| format!("{value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        if values_first {
            parts.push(format!("[{values}]"));
        }
        parts.extend(
            child_features
                .iter()
                .map(|feature| feature_constant_name(feature, suffix)),
        );
        if !values_first {
            parts.push(format!("[{values}]"));
        }
    } else {
        parts.extend(
            child_features
                .iter()
                .map(|feature| feature_constant_name(feature, suffix)),
        );
    }

    parts.join(" + ")
}

pub(crate) fn feature_constant_name(feature_name: &str, suffix: &str) -> String {
    format!("{}_{}", feature_name.to_shouty_snake_case(), suffix)
}

pub(crate) fn format_dependency_label(
    dependency: &str,
    dependency_aliases: &HashMap<String, String>,
) -> String {
    dependency_aliases
        .get(dependency)
        .cloned()
        .unwrap_or_else(|| format!("@crates//:{dependency}"))
}
