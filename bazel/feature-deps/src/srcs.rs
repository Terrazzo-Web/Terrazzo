use std::collections::HashMap;
use std::collections::hash_map;
use std::path::Path;
use std::rc::Rc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use syn::punctuated::Punctuated;

#[derive(Default)]
pub struct SrcsManager {
    parsed_files: HashMap<Rc<Path>, Rc<syn::File>>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CollectSrcsError {
    #[error("[{n}] Failed to read {file_rs:?}: {error}", n = self.name())]
    RustSrcReadError {
        file_rs: Rc<Path>,
        error: std::io::Error,
    },

    #[error("[{n}] Failed to parse {file_rs:?}: {error}", n = self.name())]
    RustSrcParseError {
        file_rs: Rc<Path>,
        error: syn::Error,
    },
}

impl SrcsManager {
    #[allow(unused)]
    pub fn collect_negative_srcs(
        &mut self,
        feature: &str,
        file_rs: Rc<Path>,
    ) -> Result<Vec<Rc<Path>>, CollectSrcsError> {
        let mut accu = vec![];
        self.collect_negative_srcs_rec(feature, file_rs, false, &mut accu)?;
        Ok(accu)
    }

    fn collect_negative_srcs_rec(
        &mut self,
        feature: &str,
        file_rs: Rc<Path>,
        parent: bool,
        accu: &mut Vec<Rc<Path>>,
    ) -> Result<(), CollectSrcsError> {
        let parsed = self.parse_rs_file(&file_rs)?;
        for item in &parsed.items {
            let syn::Item::Mod(item_mod) = item else {
                continue;
            };
            if item_mod.content.is_some() {
                continue;
            }

            let Some(submodule_file) =
                resolve_submodule_file(&file_rs, &item_mod.ident.to_string())
            else {
                continue;
            };

            let submodule_matches = parent
                || self.mod_matches(feature, &item_mod.attrs)
                || self.file_matches(feature, &submodule_file)?;
            if submodule_matches {
                accu.push(submodule_file.clone());
            }

            self.collect_negative_srcs_rec(feature, submodule_file, submodule_matches, accu)?;
        }

        Ok(())
    }

    fn parse_rs_file(&mut self, file_rs: &Rc<Path>) -> Result<Rc<syn::File>, CollectSrcsError> {
        return match self.parsed_files.entry(file_rs.clone()) {
            hash_map::Entry::Occupied(entry) => Ok(entry.get().clone()),
            hash_map::Entry::Vacant(entry) => Ok(entry.insert(handle_cache_miss(file_rs)?).clone()),
        };

        fn handle_cache_miss(file_rs: &Rc<Path>) -> Result<Rc<syn::File>, CollectSrcsError> {
            let content = std::fs::read_to_string(file_rs).map_err(move |error| {
                CollectSrcsError::RustSrcReadError {
                    file_rs: file_rs.clone(),
                    error,
                }
            })?;
            syn::parse_file(&content)
                .map_err(move |error| CollectSrcsError::RustSrcParseError {
                    file_rs: file_rs.clone(),
                    error,
                })
                .map(Rc::from)
        }
    }

    fn mod_matches(&self, feature: &str, attrs: &[syn::Attribute]) -> bool {
        attrs
            .iter()
            .filter_map(cfg_feature_name)
            .any(|cfg_feature| cfg_feature != feature)
    }

    fn file_matches(
        &mut self,
        feature: &str,
        file_rs: &Rc<Path>,
    ) -> Result<bool, CollectSrcsError> {
        let parsed = self.parse_rs_file(file_rs)?;
        Ok(parsed
            .attrs
            .iter()
            .filter_map(cfg_feature_name)
            .any(|cfg_feature| cfg_feature != feature))
    }
}

fn cfg_feature_name(attr: &syn::Attribute) -> Option<String> {
    if !attr.path().is_ident("cfg") {
        return None;
    }

    let meta = &attr.meta;
    let syn::Meta::List(meta_list) = meta else {
        return None;
    };

    let nested = meta_list
        .parse_args_with(Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated);
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

fn resolve_submodule_file(parent_file: &Path, module_name: &str) -> Option<Rc<Path>> {
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
        return Some(candidate_rs.into());
    }

    let candidate_mod_rs = parent_dir.join(module_name).join("mod.rs");
    if candidate_mod_rs.exists() {
        return Some(candidate_mod_rs.into());
    }

    None
}

#[cfg(test)]
mod tests {

    use tempfile::tempdir;

    use super::SrcsManager;

    #[test]
    fn excludes_submodule_when_mod_stmt_targets_other_feature() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        std::fs::write(
            &lib_rs,
            r#"
#[cfg(feature = "server")]
mod server;
"#,
        )
        .unwrap();
        std::fs::write(&server_rs, "pub fn handler() {}").unwrap();

        let mut manager = SrcsManager::default();
        let excluded = manager
            .collect_negative_srcs("client", lib_rs.into())
            .unwrap();

        assert_eq!(excluded, vec![server_rs.into()]);
    }

    #[test]
    fn excludes_submodule_when_child_file_targets_other_feature() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        std::fs::write(
            &lib_rs,
            r#"
mod server;
"#,
        )
        .unwrap();
        std::fs::write(
            &server_rs,
            r#"
#![cfg(feature = "server")]

pub fn handler() {}
"#,
        )
        .unwrap();

        let mut manager = SrcsManager::default();
        let excluded = manager
            .collect_negative_srcs("client", lib_rs.into())
            .unwrap();

        assert_eq!(excluded, vec![server_rs.into()]);
    }

    #[test]
    fn excludes_descendants_of_excluded_submodule_recursively() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let server_rs = dir.path().join("server.rs");
        let nested_dir = dir.path().join("server");
        let http_rs = nested_dir.join("http.rs");
        std::fs::create_dir(&nested_dir).unwrap();
        std::fs::write(
            &lib_rs,
            r#"
#[cfg(feature = "server")]
mod server;
"#,
        )
        .unwrap();
        std::fs::write(
            &server_rs,
            r#"
mod http;
"#,
        )
        .unwrap();
        std::fs::write(&http_rs, "pub fn route() {}").unwrap();

        let mut manager = SrcsManager::default();
        let excluded = manager
            .collect_negative_srcs("client", lib_rs.into())
            .unwrap();

        assert_eq!(excluded, vec![server_rs.into(), http_rs.into()]);
    }

    #[test]
    fn keeps_matching_feature_and_inline_modules_out_of_results() {
        let dir = tempdir().unwrap();
        let lib_rs = dir.path().join("lib.rs");
        let client_rs = dir.path().join("client.rs");
        std::fs::write(
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
        std::fs::write(&client_rs, "pub fn handler() {}").unwrap();

        let mut manager = SrcsManager::default();
        let excluded = manager
            .collect_negative_srcs("client", lib_rs.into())
            .unwrap();

        assert!(excluded.is_empty());
    }
}
