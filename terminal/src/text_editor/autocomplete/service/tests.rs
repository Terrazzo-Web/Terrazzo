use std::path::Path;

use fluent_asserter::prelude::*;
use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;

use super::Options;

#[test]
fn exact_match() {
    enable_tracing_for_tests();
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    assert_that!(&root).ends_with("/terminal");

    let autocomplete = call_autocomplete(&root, format!("{root}/src/text_editor"));
    assert_that!(&autocomplete).contains(&"ROOT/src".into());
    assert_that!(&autocomplete).contains(&"ROOT/src/text_editor/autocomplete".into());
    assert_that!(&autocomplete).contains(&"ROOT/src/text_editor/manager.rs".into());
    assert_that!(&autocomplete).contains(&"ROOT/src/text_editor/path_selector".into());
    assert_that!(&autocomplete).does_not_contain_any(&[&"ROOT/xyz".into()]);
}

#[test]
fn fuzzy_match() {
    enable_tracing_for_tests();
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let autocomplete = call_autocomplete(&root, format!("{root}/uild"));
    assert_that!(&autocomplete).is_not_empty();
    assert_that!(&autocomplete).contains(&"ROOT/build.rs".into());

    const SERVICE_PATH: &str = "ROOT/src/text_editor/path_selector/service.rs";
    const UI_PATH: &str = "ROOT/src/text_editor/path_selector/ui.rs";
    const PARENT_PATH: &str = "ROOT/src/text/path";

    let autocomplete = call_autocomplete(&root, format!("{root}/src/text/path/ui"));
    assert_that!(&autocomplete).is_not_empty();
    assert_that!(&autocomplete).does_not_contain_any(&[&SERVICE_PATH.into()]);
    assert_that!(&autocomplete).contains(&UI_PATH.into());
    assert_that!(&autocomplete).contains(&PARENT_PATH.into());

    let autocomplete = call_autocomplete(&root, format!("{root}/src/text/path/rs"));
    assert_that!(&autocomplete).is_not_empty();
    assert_that!(&autocomplete).contains(&SERVICE_PATH.into());
    assert_that!(&autocomplete).contains(&UI_PATH.into());
    assert_that!(&autocomplete).contains(&PARENT_PATH.into());
}

#[test]
fn match_dirs() {
    enable_tracing_for_tests();
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let autocomplete = call_autocomplete_dir(&root, format!("{root}/src/text/e"));
    assert_that!(&autocomplete).is_equal_to(
        &[
            "ROOT/src/text",
            "ROOT/src/text_editor/autocomplete",
            "ROOT/src/text_editor/path_selector",
            "ROOT/src/text_editor/search",
            "ROOT/src/text_editor/side",
        ]
        .map(Into::into)
        .into(),
    );
}

#[test]
fn match_files() {
    enable_tracing_for_tests();
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let autocomplete = call_autocomplete_files(&root, format!("{root}/src/text/u"));
    assert_that!(&autocomplete).is_equal_to(
        &["text", "text_editor/rust_lang.rs", "text_editor/ui.rs"]
            .map(Into::into)
            .into(),
    );
}

fn call_autocomplete(prefix: &str, path: String) -> Vec<String> {
    super::autocomplete_path_impl("".as_ref(), Path::new(&path), Options::default(), |_| true)
        .unwrap()
        .into_iter()
        .map(|p| p.path.replace(prefix, "ROOT"))
        .collect()
}

fn call_autocomplete_dir(prefix: &str, path: String) -> Vec<String> {
    super::autocomplete_path_impl("".as_ref(), Path::new(&path), Options::default(), |m| {
        m.is_dir()
    })
    .unwrap()
    .into_iter()
    .map(|p| p.path.replace(prefix, "ROOT"))
    .collect()
}

fn call_autocomplete_files(prefix: &str, path: String) -> Vec<String> {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    super::autocomplete_path_impl(
        format!("{root}/src").as_ref(),
        Path::new(&path),
        Options::default(),
        |m| m.is_file(),
    )
    .unwrap()
    .iter_mut()
    .map(|p| p.path.replace(prefix, "ROOT"))
    .collect()
}
