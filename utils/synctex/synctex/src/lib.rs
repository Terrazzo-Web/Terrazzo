use std::ffi::CStr;

use terrazzo_synctex_sys as sys;

mod r#box;
mod error;
mod node;
mod query_results;
mod scanner;

pub use r#box::TexBox;
pub use r#box::VisibleBox;
pub use error::Error;
pub use node::Node;
pub use query_results::QueryResults;
pub use scanner::Scanner;

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn status_to_result(status: sys::synctex_status_t) -> Result<usize> {
    if status < 0 {
        Err(Error::Status(status))
    } else {
        Ok(status as usize)
    }
}

pub(crate) unsafe fn optional_cstr<'a>(value: *const std::os::raw::c_char) -> Option<&'a CStr> {
    if value.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(value) })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use super::Scanner;

    static TEST_FIXTURES: &str = "tests/fixtures/edit_query";

    fn fixture_dir() -> PathBuf {
        let result = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TEST_FIXTURES);
        if result.exists() {
            return result;
        }

        return runfiles::find_runfiles_dir()
            .unwrap()
            .join(std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE"))
            .join(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join(TEST_FIXTURES);
    }

    fn copy_fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        for name in ["1.pdf", "1.synctex", "1.tex"] {
            let from = fixture_dir().join(name);
            let to = temp.path().join(name);
            std::fs::copy(&from, &to)
                .inspect_err(|error| eprintln!("Failed to copy from {from:?} to {to:?}: {error}"))
                .unwrap();
        }
        temp
    }

    #[test]
    fn missing_output_fails_to_open() {
        let err = Scanner::open(Path::new("/definitely/missing/output.pdf"), None).unwrap_err();
        assert!(matches!(err, super::Error::OpenFailed));
    }

    #[test]
    fn parses_fixture_and_exposes_metadata() {
        let temp = copy_fixture();
        let scanner = Scanner::open(&temp.path().join("1.pdf"), None).unwrap();

        assert_eq!(scanner.output_format().unwrap().to_str().unwrap(), "pdf");
        assert!(scanner.magnification() > 0.0);
        assert_eq!(scanner.x_offset(), 0);
        assert_eq!(scanner.y_offset(), 0);
        assert!(
            scanner
                .output()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("1.pdf")
        );
        assert!(
            scanner
                .synctex()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("1.synctex")
        );
        assert_eq!(
            scanner.name_for_tag(252).unwrap().to_str().unwrap(),
            "./1.tex"
        );
        assert!(scanner.input().is_some());
        assert!(scanner.input_with_tag(252).is_some());
        assert!(scanner.sheet(1).is_some());
        assert!(scanner.sheet_content(1).is_some());
    }

    #[test]
    fn display_query_iterates_results() {
        let temp = copy_fixture();
        let mut scanner = Scanner::open(&temp.path().join("1.pdf"), None).unwrap();
        let mut results = scanner
            .display_query(Path::new("./1.tex"), 23, 30, 1)
            .unwrap();

        assert!(!results.is_empty());
        let node = results.next().unwrap();
        assert_eq!(node.tag(), 252);
        assert_eq!(node.line(), 23);
        assert_eq!(node.page(), 1);
        assert_eq!(node.name().unwrap().to_str().unwrap(), "./1.tex");
        assert!(node.visible_box().width >= 0.0);
        assert!(node.tex_box().width >= 0);
        assert!(results.all(|node| node.line() == 23));
    }

    #[test]
    fn edit_query_iterates_and_resets_results() {
        let temp = copy_fixture();
        let mut scanner = Scanner::open(&temp.path().join("1.pdf"), None).unwrap();
        let mut results = scanner.edit_query(1, 30.0, 50.0).unwrap();

        assert!(!results.is_empty());
        let first = results.next().unwrap();
        assert_eq!(first.tag(), 252);
        assert_eq!(first.line(), 23);
        results.reset().unwrap();
        assert_eq!(results.next().unwrap().line(), 23);
    }
}
