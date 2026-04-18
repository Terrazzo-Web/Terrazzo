use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use syn::punctuated::Punctuated;

pub struct SrcsManager<'a> {
    root_rs: &'a Path,
    all_srcs: Vec<PathBuf>,
    parsed_files: HashMap<PathBuf, FileIdx>,
    prev_excluded_srcs: HashSet<usize>,
    unprocessed_features: Vec<&'a str>,
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
        file_rs: PathBuf,
        error: std::io::Error,
    },

    #[error("[{n}] Failed to parse {file_rs:?}: {error}", n = self.name())]
    RustSrcParseError { file_rs: PathBuf, error: syn::Error },

    #[error("[{n}] File not found: {0:?}", n = self.name())]
    RustSrcNotFound(PathBuf),
}

impl<'a> SrcsManager<'a> {
    pub fn new(
        root_rs: &'a Path,
        all_srcs: Vec<PathBuf>,
        unprocessed_features: &'a [String],
    ) -> Self {
        Self {
            root_rs,
            all_srcs,
            parsed_files: Default::default(),
            prev_excluded_srcs: Default::default(),
            unprocessed_features: unprocessed_features.iter().map(|s| s.as_str()).collect(),
        }
    }

    pub fn emit_all_excluded_srcs(mut self, output: &mut String) -> Result<(), CollectSrcsError> {
        for file_rs in std::mem::take(&mut self.all_srcs) {
            self.add_rs_file(&file_rs)?;
        }
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
            self.unprocessed_features.retain(|f| *f != min_feature);
            self.prev_excluded_srcs = min_excluded_srcs;
        }
        output.push_str("]\n");
        Ok(())
    }

    fn emit_excluded_srcs(
        &self,
        output: &mut String,
        feature: &str,
        delta: Vec<i32>,
    ) -> Result<(), CollectSrcsError> {
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
        self.collect_excluded_srcs(feature, &self.root_rs, false, &mut excluded_srcs)?;
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
        Ok((excluded_srcs, delta_encode(delta)))
    }

    fn collect_excluded_srcs(
        &self,
        feature: &str,
        file_rs: &Path,
        parent: bool,
        excluded_srcs_accu: &mut Vec<usize>,
    ) -> Result<(), CollectSrcsError> {
        let parsed = self.get_rs_file(&file_rs)?;
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
                excluded_srcs_accu.push(self.get_rs_file(&submodule_file)?.idx);
            }

            self.collect_excluded_srcs(
                feature,
                &submodule_file,
                submodule_matches || self.file_matches(feature, &submodule_file)?,
                excluded_srcs_accu,
            )?;
        }

        Ok(())
    }

    fn get_rs_file(&self, file_rs: &Path) -> Result<&FileIdx, CollectSrcsError> {
        self.parsed_files
            .get(file_rs)
            .ok_or_else(|| CollectSrcsError::RustSrcNotFound(file_rs.into()))
    }

    fn add_rs_file(&mut self, file_rs: &Path) -> Result<(), CollectSrcsError> {
        self.parsed_files.insert(
            file_rs.into(),
            FileIdx {
                idx: self.parsed_files.len() + 1,
                content: load(file_rs)?,
            },
        );

        return Ok(());

        fn load(file_rs: &Path) -> Result<syn::File, CollectSrcsError> {
            let content = std::fs::read_to_string(file_rs).map_err(move |error| {
                CollectSrcsError::RustSrcReadError {
                    file_rs: file_rs.into(),
                    error,
                }
            })?;
            syn::parse_file(&content).map_err(move |error| CollectSrcsError::RustSrcParseError {
                file_rs: file_rs.into(),
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
        let parsed = self.get_rs_file(file_rs)?;
        let attrs = &parsed.content.attrs;
        Ok(attrs
            .iter()
            .filter_map(cfg_feature_name)
            .any(|cfg_feature| cfg_feature == feature))
    }
}

fn delta_encode(mut delta: Vec<i32>) -> Vec<i32> {
    delta.sort();
    if delta.is_empty() {
        return vec![];
    }
    let mut result = vec![];

    let mut seq_len = 1;
    for ij in delta.windows(2) {
        let i = ij[0];
        let j = ij[1];
        let diff = j - i;
        if diff == 1 {
            seq_len += 1;
            continue;
        }
        result.push(i - seq_len + 1);
        result.push(seq_len);

        seq_len = 1;
    }

    result.push(*delta.last().unwrap() - seq_len + 1);
    result.push(seq_len);
    result
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
    use super::delta_encode;

    #[test]
    fn delta_encode_empty() {
        assert_eq!(delta_encode(vec![]), vec![]);
    }

    #[test]
    fn delta_encode_single() {
        assert_eq!(delta_encode(vec![17]), vec![17, 1]);
    }

    #[test]
    fn delta_encode_1_1() {
        assert_eq!(delta_encode(vec![8, 10]), vec![8, 1, 10, 1]);
    }

    #[test]
    fn delta_encode_2() {
        assert_eq!(delta_encode(vec![8, 9]), vec![8, 2]);
    }

    #[test]
    fn delta_encode_test() {
        assert_eq!(
            delta_encode(vec![8, 9, 10, 15, 17, 19]),
            vec![8, 3, 15, 1, 17, 1, 19, 1]
        );
    }
}
