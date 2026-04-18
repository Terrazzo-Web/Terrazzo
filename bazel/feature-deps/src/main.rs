use clap::Parser;

mod args;
mod error;
mod manager;
mod srcs;

use args::Args;
use error::FeatureDepsError;
use manager::Manager;

fn main() -> Result<(), FeatureDepsError> {
    let args = Args::parse();
    let features = args.parse_features()?;
    let dependency_aliases = args.parse_dependency_aliases()?;
    let dependency_exclusion = args.dependency_exclusion();
    let output = Manager::new(
        args.package_name,
        args.root_rs.into(),
        features,
        dependency_aliases,
        dependency_exclusion,
    )
    .render_bzl()?;
    std::fs::write(&args.output_bzl, output)
        .map_err(|error| format!("failed to write {}: {error}", args.output_bzl.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::io::Write;

    use clap::Parser;
    use tempfile::NamedTempFile;

    use crate::args::Args;
    use crate::manager::Manager;
    use crate::manager::RenderBzlError;

    fn parse_features(
        manifest: &str,
    ) -> Result<HashMap<String, Vec<String>>, crate::args::ParseFeaturesError> {
        let mut cargo_toml = NamedTempFile::new().unwrap();
        write!(cargo_toml, "{manifest}").unwrap();
        Args::parse_from([
            "feature-deps",
            cargo_toml.path().to_str().unwrap(),
            "out.bzl",
        ])
        .parse_features()
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

        let output = Manager::new(features, HashMap::new(), HashSet::new())
            .render_bzl()
            .unwrap();

        assert_eq!(
            output,
            r#""""Generated feature dependency constants."""

CLIENT_DEPS = ["@crates//:stylance"]
CLIENT_FEATURES = ["client"]
TERMINAL_DEPS = CLIENT_DEPS + ["@crates//:scopeguard"]
TERMINAL_FEATURES = CLIENT_FEATURES + ["terminal"]
"#
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

        let output = Manager::new(features, HashMap::new(), HashSet::new())
            .render_bzl()
            .unwrap();

        assert_eq!(
            output,
            r#""""Generated feature dependency constants."""

DEBUG_DEPS = []
DEBUG_FEATURES = ["debug"]
"#
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

        let error = Manager::new(features, HashMap::new(), HashSet::new())
            .render_bzl()
            .unwrap_err();

        assert!(matches!(
            error,
            RenderBzlError::FeatureNotFound { feature_name } if feature_name == "missing"
        ));
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
        let output = Manager::new(features, HashMap::new(), excluded)
            .render_bzl()
            .unwrap();

        assert_eq!(
            output,
            r#""""Generated feature dependency constants."""

SERVER_DEPS = []
SERVER_FEATURES = ["server"]
"#
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
        let output = Manager::new(features, dependency_aliases, HashSet::new())
            .render_bzl()
            .unwrap();

        assert_eq!(
            output,
            r#""""Generated feature dependency constants."""

SERVER_DEPS = ["//pty",
"//remote/client"]
SERVER_FEATURES = ["server"]
"#
        );
    }
}
