use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map;
use std::path::Path;
use std::rc::Rc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use syn::punctuated::Punctuated;

pub struct SrcsManager<'a> {
    parsed_files: RefCell<HashMap<Rc<Path>, Rc<FileIdx>>>,
    prev_excluded_srcs: HashSet<usize>,
    unprocessed_features: HashSet<&'a str>,
    root_rs: Rc<Path>,
}

struct FileIdx {
    idx: usize,
    content: syn::File,
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

impl<'a> SrcsManager<'a> {
    pub fn new(unprocessed_features: &'a [String], root_rs: Rc<Path>) -> Self {
        Self {
            parsed_files: Default::default(),
            prev_excluded_srcs: Default::default(),
            unprocessed_features: unprocessed_features.iter().map(|s| s.as_str()).collect(),
            root_rs,
        }
    }

    pub fn emit_all_excluded_srcs(&mut self, output: &mut String) -> Result<(), CollectSrcsError> {
        output.push_str("_EXCLUSION_MAP = [");
        while !self.unprocessed_features.is_empty() {
            let mut min_accu: Option<(Vec<i32>, HashSet<usize>, &str)> = None;
            for feature in &self.unprocessed_features {
                let (excluded_srcs, delta) = self.find_excluded_srcs(feature)?;
                match &mut min_accu {
                    Some(min_delta) => {
                        if delta.len() < min_delta.0.len() {
                            *min_delta = (delta, excluded_srcs, feature)
                        }
                    }
                    None => min_accu = Some((delta, excluded_srcs, feature)),
                }
            }
            let (min_delta, min_excluded_srcs, min_feature) = min_accu.unwrap();
            self.emit_excluded_srcs(output, min_feature, min_delta)?;
            self.unprocessed_features.remove(min_feature);
            self.prev_excluded_srcs = min_excluded_srcs;
        }
        output.push_str("]\n");
        Ok(())
    }

    fn emit_excluded_srcs(
        &self,
        output: &mut String,
        feature: &str,
        mut delta: Vec<i32>,
    ) -> Result<(), CollectSrcsError> {
        delta.sort();
        let delta = delta
            .into_iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>();
        output.push_str(&format!(
            "  {{ \"feature\":{:?}, \"delta\":[{}] }},\n",
            feature,
            delta.join(",")
        ));
        Ok(())
    }

    fn find_excluded_srcs(
        &self,
        feature: &str,
    ) -> Result<(HashSet<usize>, Vec<i32>), CollectSrcsError> {
        let mut excluded_srcs = vec![];
        self.collect_excluded_srcs(feature, self.root_rs.clone(), false, &mut excluded_srcs)?;
        let excluded_srcs = excluded_srcs.iter().cloned().collect::<HashSet<_>>();
        let add = excluded_srcs
            .iter()
            .filter(|idx| !self.prev_excluded_srcs.contains(idx))
            .cloned();
        let del = self
            .prev_excluded_srcs
            .iter()
            .filter(|idx| !excluded_srcs.contains(idx))
            .cloned();
        let delta = add
            .map(|idx| idx as i32)
            .chain(del.map(|idx| -(idx as i32)))
            .collect::<Vec<_>>();
        Ok((excluded_srcs, delta))
    }

    fn collect_excluded_srcs(
        &self,
        feature: &str,
        file_rs: Rc<Path>,
        parent: bool,
        excluded_srcs_accu: &mut Vec<usize>,
    ) -> Result<(), CollectSrcsError> {
        let parsed = self.parse_rs_file(&file_rs)?;
        let parsed = &parsed.content;
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

            let submodule_matches = parent || self.mod_matches(feature, &item_mod.attrs);
            if submodule_matches {
                excluded_srcs_accu.push(self.parse_rs_file(&submodule_file)?.idx);
            }

            self.collect_excluded_srcs(
                feature,
                submodule_file.clone(),
                submodule_matches || self.file_matches(feature, &submodule_file)?,
                excluded_srcs_accu,
            )?;
        }

        Ok(())
    }

    fn parse_rs_file(&self, file_rs: &Rc<Path>) -> Result<Rc<FileIdx>, CollectSrcsError> {
        let mut parsed_files = self.parsed_files.borrow_mut();
        let next_idx = parsed_files.len() + 1;
        return match parsed_files.entry(file_rs.clone()) {
            hash_map::Entry::Occupied(entry) => Ok(entry.get().clone()),
            hash_map::Entry::Vacant(entry) => Ok(entry
                .insert(Rc::new(FileIdx {
                    idx: next_idx,
                    content: handle_cache_miss(file_rs)?,
                }))
                .clone()),
        };

        fn handle_cache_miss(file_rs: &Rc<Path>) -> Result<syn::File, CollectSrcsError> {
            let content = std::fs::read_to_string(file_rs).map_err(move |error| {
                CollectSrcsError::RustSrcReadError {
                    file_rs: file_rs.clone(),
                    error,
                }
            })?;
            syn::parse_file(&content).map_err(move |error| CollectSrcsError::RustSrcParseError {
                file_rs: file_rs.clone(),
                error,
            })
        }
    }

    fn mod_matches(&self, feature: &str, attrs: &[syn::Attribute]) -> bool {
        attrs
            .iter()
            .filter_map(cfg_feature_name)
            .any(|cfg_feature| cfg_feature == feature)
    }

    fn file_matches(&self, feature: &str, file_rs: &Rc<Path>) -> Result<bool, CollectSrcsError> {
        let parsed = self.parse_rs_file(file_rs)?;
        let attrs = &parsed.content.attrs;
        Ok(attrs
            .iter()
            .filter_map(cfg_feature_name)
            .any(|cfg_feature| cfg_feature == feature))
    }

    pub fn emit_all_srcs(&self, output: &mut String) {
        let parsed_files = self.parsed_files.borrow();
        let mut all_files = parsed_files
            .iter()
            .map(|(path, file)| (file.idx, path))
            .collect::<Vec<_>>();
        all_files.sort_by_key(|e| e.0);
        let all_files = all_files
            .iter()
            .map(|(_, path)| format!("{path:?}"))
            .collect::<Vec<_>>();
        output.push_str(&format!("_ALL_SRCS = [{}]\n", all_files.join(",")));
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
            .collect_excluded_srcs("client", lib_rs.into())
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
            .collect_excluded_srcs("client", lib_rs.into())
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
            .collect_excluded_srcs("client", lib_rs.into())
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
            .collect_excluded_srcs("client", lib_rs.into())
            .unwrap();

        assert!(excluded.is_empty());
    }
}
