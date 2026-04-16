use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::common::collect_child_features;
use crate::common::feature_constant_name;
use crate::common::format_dependency_label;
use crate::common::render_expression;

pub(crate) fn emit_feature_constants(
    output: &mut String,
    entries: &[String],
    dependency_aliases: &HashMap<String, String>,
    dependency_exclusion: &HashSet<String>,
    feature_name: &str,
) {
    let child_features = collect_child_features(entries);
    let mut dependencies = BTreeSet::new();

    for entry in entries {
        if let Some(dependency) = entry.strip_prefix("dep:") {
            if !dependency_exclusion.contains(dependency) {
                dependencies.insert(format_dependency_label(dependency, dependency_aliases));
            }
        }
    }

    let child_feature_names = child_features.into_iter().collect::<Vec<_>>();
    let deps_expression = render_expression(&child_feature_names, &dependencies, "DEPS", false);
    let features_expression = render_expression(
        &child_feature_names,
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
}
