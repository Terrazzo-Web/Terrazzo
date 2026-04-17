use std::path::Path;
use std::path::PathBuf;

pub fn definitely_excluded_srcs(
    feature: &str,
    file_rs: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, String> {
    let file_rs = file_rs.as_ref();
    let mut excluded = Vec::new();
    collect_definitely_excluded_srcs(feature, file_rs, false, &mut excluded)?;
    Ok(excluded)
}

fn collect_definitely_excluded_srcs(
    feature: &str,
    file_rs: &Path,
    parent_excluded: bool,
    excluded: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let parsed = parse_rs_file(file_rs)?;
    for item in parsed.items {
        let syn::Item::Mod(item_mod) = item else {
            continue;
        };
        if item_mod.content.is_some() {
            continue;
        }

        let Some(submodule_file) = resolve_submodule_file(file_rs, &item_mod.ident.to_string()) else {
            continue;
        };

        let submodule_excluded = parent_excluded
            || mod_is_definitely_excluded(feature, &item_mod.attrs)
            || file_is_definitely_excluded(feature, &submodule_file)?;
        if submodule_excluded {
            excluded.push(submodule_file.clone());
        }
        collect_definitely_excluded_srcs(feature, &submodule_file, submodule_excluded, excluded)?;
    }

    Ok(())
}

fn mod_is_definitely_excluded(feature: &str, attrs: &[syn::Attribute]) -> bool {
    attrs.iter()
        .filter_map(cfg_feature_name)
        .any(|cfg_feature| cfg_feature != feature)
}

fn file_is_definitely_excluded(feature: &str, file_rs: &Path) -> Result<bool, String> {
    let parsed = parse_rs_file(file_rs)?;
    Ok(parsed
        .attrs
        .iter()
        .filter_map(cfg_feature_name)
        .any(|cfg_feature| cfg_feature != feature))
}

fn parse_rs_file(file_rs: &Path) -> Result<syn::File, String> {
    let content = std::fs::read_to_string(file_rs)
        .map_err(|error| format!("failed to read {}: {error}", file_rs.display()))?;
    syn::parse_file(&content).map_err(|error| format!("failed to parse {}: {error}", file_rs.display()))
}

fn cfg_feature_name(attr: &syn::Attribute) -> Option<String> {
    if !attr.path().is_ident("cfg") {
        return None;
    }

    let meta = &attr.meta;
    let syn::Meta::List(meta_list) = meta else {
        return None;
    };

    let nested = meta_list.parse_args_with(
        syn::punctuated::Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated,
    );
    let Ok(nested) = nested else {
        return None;
    };
    if nested.len() != 1 {
        return None;
    }

    let item = nested.first()?;
    if !item.path.is_ident("feature") {
        return None;
    }

    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(value),
        ..
    }) = &item.value
    else {
        return None;
    };

    Some(value.value())
}

fn resolve_submodule_file(parent_file: &Path, module_name: &str) -> Option<PathBuf> {
    let parent_dir = if parent_file
        .file_stem()
        .is_some_and(|stem| stem == "mod" || stem == "lib" || stem == "main")
    {
        parent_file.parent()?.to_path_buf()
    } else {
        parent_file.parent()?.join(parent_file.file_stem()?)
    };

    let candidate_rs = parent_dir.join(format!("{module_name}.rs"));
    if candidate_rs.exists() {
        return Some(candidate_rs);
    }

    let candidate_mod_rs = parent_dir.join(module_name).join("mod.rs");
    if candidate_mod_rs.exists() {
        return Some(candidate_mod_rs);
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::definitely_excluded_srcs;

    #[test]
    fn excludes_submodule_when_mod_stmt_targets_other_feature() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        fs::write(
            &lib_rs,
            r#"
#[cfg(feature = "server")]
mod server;
"#,
        )
        .unwrap();
        fs::write(&server_rs, "pub fn handler() {}").unwrap();

        let excluded = definitely_excluded_srcs("client", &lib_rs).unwrap();

        assert_eq!(excluded, vec![server_rs]);
    }

    #[test]
    fn excludes_submodule_when_child_file_targets_other_feature() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        fs::write(
            &lib_rs,
            r#"
mod server;
"#,
        )
        .unwrap();
        fs::write(
            &server_rs,
            r#"
#![cfg(feature = "server")]

pub fn handler() {}
"#,
        )
        .unwrap();

        let excluded = definitely_excluded_srcs("client", &lib_rs).unwrap();

        assert_eq!(excluded, vec![server_rs]);
    }

    #[test]
    fn excludes_descendants_of_excluded_submodule_recursively() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        let nested_dir = dir.path().join("server");
        let http_rs = nested_dir.join("http.rs");
        fs::create_dir(&nested_dir).unwrap();
        fs::write(
            &lib_rs,
            r#"
#[cfg(feature = "server")]
mod server;
"#,
        )
        .unwrap();
        fs::write(
            &server_rs,
            r#"
mod http;
"#,
        )
        .unwrap();
        fs::write(&http_rs, "pub fn route() {}").unwrap();

        let excluded = definitely_excluded_srcs("client", &lib_rs).unwrap();

        assert_eq!(excluded, vec![server_rs, http_rs]);
    }

    #[test]
    fn keeps_matching_feature_and_inline_modules_out_of_results() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let client_rs = dir.path().join("client.rs");
        fs::write(
            &lib_rs,
            r#"
#[cfg(feature = "client")]
mod client;

mod inline_only {
    pub fn helper() {}
}
"#,
        )
        .unwrap();
        fs::write(&client_rs, "pub fn handler() {}").unwrap();

        let excluded = definitely_excluded_srcs("client", &lib_rs).unwrap();

        assert!(excluded.is_empty());
    }
}
