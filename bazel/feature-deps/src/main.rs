use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use clap::Parser;

mod common;
mod deps_generator;
mod srcs_generator;

use common::SourceLayout;
use common::parse_dependency_aliases;
use common::parse_manifest;
use deps_generator::emit_feature_constants;
use srcs_generator::emit_default_srcs;
use srcs_generator::emit_feature_srcs;

#[cfg(test)]
use srcs_generator::collect_feature_local_srcs;

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
    let manifest_data = parse_manifest(&manifest)?;
    let dependency_aliases = parse_dependency_aliases(args.dependency_aliases)?;
    let dependency_exclusion = args
        .dependency_exclusion
        .into_iter()
        .collect::<HashSet<_>>();
    let source_layout = SourceLayout::new(&args.cargo_toml, &manifest_data.lib_rs_path)?;
    let output = render_bzl(
        &manifest_data.features,
        &source_layout,
        &dependency_aliases,
        &dependency_exclusion,
    )?;
    fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}

fn render_bzl(
    features: &HashMap<String, Vec<String>>,
    source_layout: &SourceLayout,
    dependency_aliases: &HashMap<String, String>,
    dependency_exclusion: &HashSet<String>,
) -> Result<String, String> {
    let mut output = String::from("\"\"\"Generated feature dependency constants.\"\"\"\n\n");
    let mut emitted = HashSet::new();

    emit_default_srcs(&mut output, features, source_layout)?;

    let mut feature_names = features.keys().cloned().collect::<Vec<_>>();
    feature_names.sort();

    for feature_name in feature_names {
        emit_feature(
            &mut output,
            &mut emitted,
            features,
            source_layout,
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
    source_layout: &SourceLayout,
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

    for entry in entries {
        if entry.starts_with("dep:") || entry.contains('/') {
            continue;
        }
        emit_feature(
            output,
            emitted,
            features,
            source_layout,
            dependency_aliases,
            dependency_exclusion,
            entry,
        )?;
    }

    emit_feature_constants(
        output,
        entries,
        dependency_aliases,
        dependency_exclusion,
        feature_name,
    );
    emit_feature_srcs(output, entries, source_layout, feature_name)?;
    emitted.insert(feature_name.to_owned());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::SourceLayout;
    use super::collect_feature_local_srcs;
    use super::common::parse_features;
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
        let temp = TestCrate::new(
            r#"
mod client;
mod terminal;
"#,
        )
        .file("client.rs", "")
        .file("terminal.rs", "")
        .build();

        let output = render_bzl(&features, &temp.layout, &HashMap::new(), &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\
\"\"\"Generated feature dependency constants.\"\"\"\n\
\n\
DEFAULT_SRCS = [\"client.rs\",\n\"lib.rs\",\n\"terminal.rs\"]\n\
CLIENT_DEPS = [\"@crates//:stylance\"]\n\
CLIENT_FEATURES = [\"client\"]\n\
CLIENT_SRCS = DEFAULT_SRCS + []\n\
TERMINAL_DEPS = CLIENT_DEPS + [\"@crates//:scopeguard\"]\n\
TERMINAL_FEATURES = [\"terminal\"] + CLIENT_FEATURES\n\
TERMINAL_SRCS = CLIENT_SRCS + []\n"
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
        let temp = TestCrate::new("").build();

        let output = render_bzl(&features, &temp.layout, &HashMap::new(), &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nDEFAULT_SRCS = [\"lib.rs\"]\nDEBUG_DEPS = []\nDEBUG_FEATURES = [\"debug\"]\nDEBUG_SRCS = DEFAULT_SRCS + []\n"
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
        let temp = TestCrate::new("").build();

        let error =
            render_bzl(&features, &temp.layout, &HashMap::new(), &HashSet::new()).unwrap_err();

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
        let temp = TestCrate::new("").build();

        let excluded = HashSet::from(["terrazzo-pty".to_owned(), "trz-gateway-client".to_owned()]);
        let output = render_bzl(&features, &temp.layout, &HashMap::new(), &excluded).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nDEFAULT_SRCS = [\"lib.rs\"]\nSERVER_DEPS = []\nSERVER_FEATURES = [\"server\"]\nSERVER_SRCS = DEFAULT_SRCS + []\n"
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
        let temp = TestCrate::new("").build();

        let dependency_aliases = HashMap::from([
            ("terrazzo-pty".to_owned(), "//pty".to_owned()),
            (
                "trz-gateway-client".to_owned(),
                "//remote/client".to_owned(),
            ),
        ]);
        let output = render_bzl(&features, &temp.layout, &dependency_aliases, &HashSet::new()).unwrap();

        assert_eq!(
            output,
            "\"\"\"Generated feature dependency constants.\"\"\"\n\nDEFAULT_SRCS = [\"lib.rs\"]\nSERVER_DEPS = [\"//pty\",\n\"//remote/client\"]\nSERVER_FEATURES = [\"server\"]\nSERVER_SRCS = DEFAULT_SRCS + []\n"
        );
    }

    #[test]
    fn feature_srcs_include_default_and_feature_specific_modules() {
        let temp = TestCrate::new(
            r#"
#[cfg(feature = "client")]
mod client;

#[cfg(feature = "server")]
mod server;

mod shared;
"#,
        )
        .file("client.rs", "mod api;\n")
        .file("client/api.rs", "")
        .file("server.rs", "#![cfg(feature = \"server\")]\n\nmod http;\n")
        .file("server/http.rs", "")
        .file("shared.rs", "mod nested;\n")
        .file("shared/nested.rs", "")
        .build();

        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "default", true).unwrap(),
            vec![
                "lib.rs".to_owned(),
                "shared.rs".to_owned(),
                "shared/nested.rs".to_owned(),
            ]
        );
        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "client", false).unwrap(),
            vec!["client.rs".to_owned(), "client/api.rs".to_owned()]
        );
        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "server", false).unwrap(),
            vec!["server.rs".to_owned(), "server/http.rs".to_owned()]
        );
    }

    #[test]
    fn no_cfg_submodule_propagates_parent_included_state() {
        let temp = TestCrate::new(
            r#"
#[cfg(feature = "client")]
mod client;
"#,
        )
        .file("client.rs", "mod nested;\n")
        .file("client/nested.rs", "")
        .build();

        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "default", true).unwrap(),
            vec!["lib.rs".to_owned()]
        );
        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "client", false).unwrap(),
            vec!["client.rs".to_owned(), "client/nested.rs".to_owned()]
        );
    }

    #[test]
    fn test_only_submodules_are_excluded() {
        let temp = TestCrate::new("mod sample;\n")
            .file("sample.rs", "mod tests;\n")
            .file("sample/tests.rs", "#![cfg(test)]\n")
            .build();

        assert_eq!(
            collect_feature_local_srcs(&temp.layout, "default", true).unwrap(),
            vec!["lib.rs".to_owned(), "sample.rs".to_owned()]
        );
    }

    struct TestCrate {
        tempdir: TempDir,
        lib_rs: String,
        files: Vec<(String, String)>,
    }

    impl TestCrate {
        fn new(lib_rs: &str) -> Self {
            Self {
                tempdir: tempfile::tempdir().unwrap(),
                lib_rs: lib_rs.to_owned(),
                files: Vec::new(),
            }
        }

        fn file(mut self, relative_path: &str, contents: &str) -> Self {
            self.files
                .push((relative_path.to_owned(), contents.to_owned()));
            self
        }

        fn build(self) -> BuiltTestCrate {
            let crate_dir = self.tempdir.path().to_path_buf();
            fs::write(
                crate_dir.join("Cargo.toml"),
                "[package]\nname = \"test-crate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            )
            .unwrap();
            write_file(&crate_dir, Path::new("src/lib.rs"), &self.lib_rs);
            for (relative_path, contents) in self.files {
                write_file(&crate_dir, Path::new("src").join(relative_path).as_path(), &contents);
            }

            BuiltTestCrate {
                _tempdir: self.tempdir,
                layout: SourceLayout::new(&crate_dir.join("Cargo.toml"), Path::new("src/lib.rs"))
                    .unwrap(),
            }
        }
    }

    struct BuiltTestCrate {
        _tempdir: TempDir,
        layout: SourceLayout,
    }

    fn write_file(crate_dir: &Path, relative_path: &Path, contents: &str) {
        let path = crate_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }
}
