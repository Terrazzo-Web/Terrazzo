use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use syn::Attribute;
use syn::Expr;
use syn::ExprLit;
use syn::File;
use syn::Item;
use syn::Lit;
use syn::Meta;
use syn::parse::Parser as _;

use crate::common::SourceLayout;
use crate::common::collect_child_features;
use crate::common::feature_constant_name;
use crate::common::render_expression;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum GuardState {
    Include,
    Exclude,
    Propagate,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CfgMatch {
    True,
    False,
    Unknown,
}

pub(crate) fn emit_default_srcs(
    output: &mut String,
    features: &HashMap<String, Vec<String>>,
    source_layout: &SourceLayout,
) -> Result<(), String> {
    let default_entries = features.get("default").map(Vec::as_slice).unwrap_or(&[]);
    let default_srcs = collect_feature_local_srcs(source_layout, "default", true)?;
    let child_feature_names = collect_child_features(default_entries)
        .into_iter()
        .collect::<Vec<_>>();
    let srcs_expression = render_expression(
        &child_feature_names,
        &default_srcs.into_iter().collect(),
        "SRCS",
        false,
    );
    output.push_str(&format!("DEFAULT_SRCS = {}\n", srcs_expression));
    Ok(())
}

pub(crate) fn emit_feature_srcs(
    output: &mut String,
    entries: &[String],
    source_layout: &SourceLayout,
    feature_name: &str,
) -> Result<(), String> {
    let child_features = collect_child_features(entries)
        .into_iter()
        .collect::<Vec<_>>();
    let local_srcs = collect_feature_local_srcs(source_layout, feature_name, false)?;
    let srcs_expression = render_feature_srcs_expression(&child_features, local_srcs, feature_name);
    output.push_str(&format!(
        "{} = {}\n",
        feature_constant_name(feature_name, "SRCS"),
        srcs_expression
    ));
    Ok(())
}

fn render_feature_srcs_expression(
    child_features: &[String],
    local_srcs: Vec<String>,
    feature_name: &str,
) -> String {
    if feature_name == "default" {
        return "DEFAULT_SRCS".to_owned();
    }

    let local_values = local_srcs.into_iter().collect::<BTreeSet<_>>();
    let mut parts = Vec::new();

    if child_features.is_empty() {
        parts.push("DEFAULT_SRCS".to_owned());
    } else {
        parts.extend(
            child_features
                .iter()
                .map(|feature| feature_constant_name(feature, "SRCS")),
        );
    }

    parts.push(render_expression(&[], &local_values, "SRCS", false));
    parts.join(" + ")
}

pub(crate) fn collect_feature_local_srcs(
    source_layout: &SourceLayout,
    feature_name: &str,
    start_included: bool,
) -> Result<Vec<String>, String> {
    let mut visited = HashSet::new();
    let mut emitted = Vec::new();
    collect_module_srcs(
        source_layout,
        feature_name,
        &source_layout.lib_rs_path,
        start_included,
        &mut visited,
        &mut emitted,
    )?;
    Ok(emitted)
}

fn collect_module_srcs(
    source_layout: &SourceLayout,
    feature_name: &str,
    file_path: &Path,
    included: bool,
    visited: &mut HashSet<PathBuf>,
    emitted: &mut Vec<String>,
) -> Result<(), String> {
    let canonical_key = file_path.to_path_buf();
    if !visited.insert(canonical_key) {
        return Ok(());
    }

    if included {
        let relative_path = file_path
            .strip_prefix(&source_layout.module_root)
            .map_err(|error| {
                format!(
                    "failed to relativize {} against {}: {error}",
                    file_path.display(),
                    source_layout.module_root.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        emitted.push(relative_path);
    }

    let file = parse_rust_file(file_path)?;
    for item in &file.items {
        let Item::Mod(item_mod) = item else {
            continue;
        };
        if item_mod.content.is_some() {
            continue;
        }

        let child_path = resolve_submodule_path(file_path, &item_mod.ident.to_string())?;
        let child_file = parse_rust_file(&child_path)?;
        let child_included =
            determine_child_included(feature_name, included, &item_mod.attrs, &child_file.attrs)?;
        collect_module_srcs(
            source_layout,
            feature_name,
            &child_path,
            child_included,
            visited,
            emitted,
        )?;
    }

    Ok(())
}

fn parse_rust_file(file_path: &Path) -> Result<File, String> {
    let source = fs::read_to_string(file_path)
        .map_err(|error| format!("failed to read {}: {error}", file_path.display()))?;
    syn::parse_file(&source)
        .map_err(|error| format!("failed to parse {}: {error}", file_path.display()))
}

fn resolve_submodule_path(file_path: &Path, module_name: &str) -> Result<PathBuf, String> {
    let Some(parent) = file_path.parent() else {
        return Err(format!(
            "could not determine parent directory for {}",
            file_path.display()
        ));
    };
    let Some(stem) = file_path.file_stem().and_then(|stem| stem.to_str()) else {
        return Err(format!(
            "could not determine file stem for {}",
            file_path.display()
        ));
    };
    let module_dir = if matches!(stem, "lib" | "mod") {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    };

    let direct_file = module_dir.join(format!("{module_name}.rs"));
    if direct_file.is_file() {
        return Ok(direct_file);
    }

    let nested_mod_file = module_dir.join(module_name).join("mod.rs");
    if nested_mod_file.is_file() {
        return Ok(nested_mod_file);
    }

    Err(format!(
        "could not resolve module {module_name:?} from {}",
        file_path.display()
    ))
}

fn determine_child_included(
    feature_name: &str,
    parent_included: bool,
    module_attrs: &[Attribute],
    file_attrs: &[Attribute],
) -> Result<bool, String> {
    let module_state = cfg_guard_state(module_attrs, feature_name)?;
    let file_state = cfg_guard_state(file_attrs, feature_name)?;

    if matches!(module_state, GuardState::Exclude) || matches!(file_state, GuardState::Exclude) {
        return Ok(false);
    }
    if matches!(module_state, GuardState::Include) || matches!(file_state, GuardState::Include) {
        return Ok(true);
    }
    Ok(parent_included)
}

fn cfg_guard_state(attrs: &[Attribute], feature_name: &str) -> Result<GuardState, String> {
    let mut saw_cfg = false;
    for attr in attrs {
        if !attr.path().is_ident("cfg") {
            continue;
        }
        saw_cfg = true;

        let Meta::List(list) = &attr.meta else {
            return Err("unsupported cfg attribute".to_owned());
        };
        let nested_meta = syn::parse2::<Meta>(list.tokens.clone())
            .map_err(|error| format!("failed to parse cfg attribute: {error}"))?;
        if matches!(eval_cfg_meta(&nested_meta, feature_name), CfgMatch::False) {
            return Ok(GuardState::Exclude);
        }
    }

    if saw_cfg {
        Ok(GuardState::Include)
    } else {
        Ok(GuardState::Propagate)
    }
}

fn eval_cfg_meta(meta: &Meta, feature_name: &str) -> CfgMatch {
    match meta {
        Meta::Path(path) => {
            if path.is_ident("test") {
                CfgMatch::False
            } else {
                CfgMatch::Unknown
            }
        }
        Meta::NameValue(name_value) => {
            if !name_value.path.is_ident("feature") {
                return CfgMatch::Unknown;
            }
            let Expr::Lit(ExprLit {
                lit: Lit::Str(lit_str),
                ..
            }) = &name_value.value
            else {
                return CfgMatch::Unknown;
            };
            if lit_str.value() == feature_name {
                CfgMatch::True
            } else {
                CfgMatch::False
            }
        }
        Meta::List(list) => {
            if list.path.is_ident("all") {
                combine_all(list, feature_name)
            } else if list.path.is_ident("any") {
                combine_any(list, feature_name)
            } else if list.path.is_ident("not") {
                negate_cfg(list, feature_name)
            } else {
                CfgMatch::Unknown
            }
        }
    }
}

fn combine_all(list: &syn::MetaList, feature_name: &str) -> CfgMatch {
    let mut saw_unknown = false;
    let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
    let nested = match parser.parse2(list.tokens.clone()) {
        Ok(nested) => nested,
        Err(_) => return CfgMatch::Unknown,
    };
    for meta in nested {
        match eval_cfg_meta(&meta, feature_name) {
            CfgMatch::False => return CfgMatch::False,
            CfgMatch::Unknown => saw_unknown = true,
            CfgMatch::True => {}
        }
    }
    if saw_unknown {
        CfgMatch::Unknown
    } else {
        CfgMatch::True
    }
}

fn combine_any(list: &syn::MetaList, feature_name: &str) -> CfgMatch {
    let mut saw_unknown = false;
    let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
    let nested = match parser.parse2(list.tokens.clone()) {
        Ok(nested) => nested,
        Err(_) => return CfgMatch::Unknown,
    };
    for meta in nested {
        match eval_cfg_meta(&meta, feature_name) {
            CfgMatch::True => return CfgMatch::True,
            CfgMatch::Unknown => saw_unknown = true,
            CfgMatch::False => {}
        }
    }
    if saw_unknown {
        CfgMatch::Unknown
    } else {
        CfgMatch::False
    }
}

fn negate_cfg(list: &syn::MetaList, feature_name: &str) -> CfgMatch {
    let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
    let nested = match parser.parse2(list.tokens.clone()) {
        Ok(nested) => nested,
        Err(_) => return CfgMatch::Unknown,
    };
    let mut nested = nested.into_iter();
    let Some(meta) = nested.next() else {
        return CfgMatch::Unknown;
    };
    if nested.next().is_some() {
        return CfgMatch::Unknown;
    }
    match eval_cfg_meta(&meta, feature_name) {
        CfgMatch::True => CfgMatch::False,
        CfgMatch::False => CfgMatch::True,
        CfgMatch::Unknown => CfgMatch::Unknown,
    }
}
