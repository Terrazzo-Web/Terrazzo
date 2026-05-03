use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::NulError;
use std::marker::PhantomData;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo_synctex_sys as sys;

pub type Result<T> = std::result::Result<T, Error>;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("[{n}] Path is not valid UTF-8", n = self.name())]
    InvalidPath,

    #[error("[{n}] Path contains an interior NUL byte: {0}", n = self.name())]
    Nul(#[from] NulError),

    #[error("[{n}] Failed to open SyncTeX scanner", n = self.name())]
    OpenFailed,

    #[error("[{n}] Failed to parse SyncTeX file", n = self.name())]
    ParseFailed,

    #[error("[{n}] SyncTeX query failed with status {0}", n = self.name())]
    Status(sys::synctex_status_t),
}

#[derive(Debug)]
pub struct Scanner {
    raw: NonNull<sys::synctex_scanner_t>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Scanner {
    pub fn open(output: &Path, build_directory: Option<&Path>) -> Result<Self> {
        Self::new_with_output_file(output, build_directory, true)
    }

    pub fn new_with_output_file(
        output: &Path,
        build_directory: Option<&Path>,
        parse: bool,
    ) -> Result<Self> {
        let output = path_to_cstring(output)?;
        let build_directory = build_directory.map(path_to_cstring).transpose()?;
        let build_directory = build_directory
            .as_ref()
            .map_or(std::ptr::null(), |path| path.as_ptr());
        let raw = unsafe {
            sys::synctex_scanner_new_with_output_file(
                output.as_ptr(),
                build_directory,
                i32::from(parse),
            )
        };
        let raw = NonNull::new(raw).ok_or(Error::OpenFailed)?;
        Ok(Self {
            raw,
            _not_send_or_sync: PhantomData,
        })
    }

    pub fn parse(&mut self) -> Result<()> {
        let raw = unsafe { sys::synctex_scanner_parse(self.raw.as_ptr()) };
        self.raw = NonNull::new(raw).ok_or(Error::ParseFailed)?;
        Ok(())
    }

    pub fn display_query(
        &mut self,
        input: &Path,
        line: i32,
        column: i32,
        page_hint: i32,
    ) -> Result<QueryResults<'_>> {
        let input = path_to_cstring(input)?;
        let status = unsafe {
            sys::synctex_display_query(self.raw.as_ptr(), input.as_ptr(), line, column, page_hint)
        };
        QueryResults::new(self, status)
    }

    pub fn edit_query(&mut self, page: i32, h: f32, v: f32) -> Result<QueryResults<'_>> {
        let status = unsafe { sys::synctex_edit_query(self.raw.as_ptr(), page, h, v) };
        QueryResults::new(self, status)
    }

    pub fn reset_result(&mut self) -> Result<()> {
        status_to_result(unsafe { sys::synctex_scanner_reset_result(self.raw.as_ptr()) }).map(drop)
    }

    pub fn name_for_tag(&self, tag: i32) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_name(self.raw.as_ptr(), tag)) }
    }

    pub fn tag_for_name(&self, name: &Path) -> Result<i32> {
        let name = path_to_cstring(name)?;
        Ok(unsafe { sys::synctex_scanner_get_tag(self.raw.as_ptr(), name.as_ptr()) })
    }

    pub fn input(&self) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_scanner_input(self.raw.as_ptr())) }
    }

    pub fn input_with_tag(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_scanner_input_with_tag(self.raw.as_ptr(), tag)) }
    }

    pub fn output(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_output(self.raw.as_ptr())) }
    }

    pub fn synctex(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_synctex(self.raw.as_ptr())) }
    }

    pub fn output_format(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_output_fmt(self.raw.as_ptr())) }
    }

    pub fn x_offset(&self) -> i32 {
        unsafe { sys::synctex_scanner_x_offset(self.raw.as_ptr()) }
    }

    pub fn y_offset(&self) -> i32 {
        unsafe { sys::synctex_scanner_y_offset(self.raw.as_ptr()) }
    }

    pub fn magnification(&self) -> f32 {
        unsafe { sys::synctex_scanner_magnification(self.raw.as_ptr()) }
    }

    pub fn sheet(&self, page: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_sheet(self.raw.as_ptr(), page)) }
    }

    pub fn sheet_content(&self, page: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_sheet_content(self.raw.as_ptr(), page)) }
    }

    pub fn form(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_form(self.raw.as_ptr(), tag)) }
    }

    pub fn form_content(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_form_content(self.raw.as_ptr(), tag)) }
    }

    pub fn display_debug(&self) {
        unsafe { sys::synctex_scanner_display(self.raw.as_ptr()) };
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        unsafe {
            sys::synctex_scanner_free(self.raw.as_ptr());
        }
    }
}

#[derive(Debug)]
pub struct QueryResults<'scanner> {
    scanner: &'scanner mut Scanner,
    count: usize,
}

impl<'scanner> QueryResults<'scanner> {
    fn new(scanner: &'scanner mut Scanner, status: sys::synctex_status_t) -> Result<Self> {
        let count = status_to_result(status)?;
        Ok(Self { scanner, count })
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn reset(&mut self) -> Result<()> {
        status_to_result(unsafe { sys::synctex_scanner_reset_result(self.scanner.raw.as_ptr()) })
            .map(drop)
    }
}

impl<'scanner> Iterator for QueryResults<'scanner> {
    type Item = Node<'scanner>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe { Node::from_raw(sys::synctex_scanner_next_result(self.scanner.raw.as_ptr())) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Node<'scanner> {
    raw: NonNull<sys::synctex_node_t>,
    _scanner: PhantomData<&'scanner Scanner>,
}

impl<'scanner> Node<'scanner> {
    unsafe fn from_raw(raw: sys::synctex_node_p) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self {
            raw,
            _scanner: PhantomData,
        })
    }

    pub fn tag(&self) -> i32 {
        unsafe { sys::synctex_node_tag(self.raw.as_ptr()) }
    }

    pub fn line(&self) -> i32 {
        unsafe { sys::synctex_node_line(self.raw.as_ptr()) }
    }

    pub fn mean_line(&self) -> i32 {
        unsafe { sys::synctex_node_mean_line(self.raw.as_ptr()) }
    }

    pub fn column(&self) -> i32 {
        unsafe { sys::synctex_node_column(self.raw.as_ptr()) }
    }

    pub fn name(&self) -> Option<&'scanner CStr> {
        unsafe { optional_cstr(sys::synctex_node_get_name(self.raw.as_ptr())) }
    }

    pub fn page(&self) -> i32 {
        unsafe { sys::synctex_node_page(self.raw.as_ptr()) }
    }

    pub fn parent(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_parent(self.raw.as_ptr())) }
    }

    pub fn parent_sheet(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_parent_sheet(self.raw.as_ptr())) }
    }

    pub fn parent_form(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_parent_form(self.raw.as_ptr())) }
    }

    pub fn child(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_child(self.raw.as_ptr())) }
    }

    pub fn last_child(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_last_child(self.raw.as_ptr())) }
    }

    pub fn sibling(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_sibling(self.raw.as_ptr())) }
    }

    pub fn last_sibling(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_last_sibling(self.raw.as_ptr())) }
    }

    pub fn arg_sibling(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_arg_sibling(self.raw.as_ptr())) }
    }

    pub fn next_node(&self) -> Option<Self> {
        unsafe { Self::from_raw(sys::synctex_node_next(self.raw.as_ptr())) }
    }

    pub fn visible(&self) -> VisibleBox {
        VisibleBox {
            h: unsafe { sys::synctex_node_visible_h(self.raw.as_ptr()) },
            v: unsafe { sys::synctex_node_visible_v(self.raw.as_ptr()) },
            width: unsafe { sys::synctex_node_visible_width(self.raw.as_ptr()) },
            height: unsafe { sys::synctex_node_visible_height(self.raw.as_ptr()) },
            depth: unsafe { sys::synctex_node_visible_depth(self.raw.as_ptr()) },
        }
    }

    pub fn visible_box(&self) -> VisibleBox {
        VisibleBox {
            h: unsafe { sys::synctex_node_box_visible_h(self.raw.as_ptr()) },
            v: unsafe { sys::synctex_node_box_visible_v(self.raw.as_ptr()) },
            width: unsafe { sys::synctex_node_box_visible_width(self.raw.as_ptr()) },
            height: unsafe { sys::synctex_node_box_visible_height(self.raw.as_ptr()) },
            depth: unsafe { sys::synctex_node_box_visible_depth(self.raw.as_ptr()) },
        }
    }

    pub fn tex(&self) -> TexBox {
        TexBox {
            h: unsafe { sys::synctex_node_h(self.raw.as_ptr()) },
            v: unsafe { sys::synctex_node_v(self.raw.as_ptr()) },
            width: unsafe { sys::synctex_node_width(self.raw.as_ptr()) },
            height: unsafe { sys::synctex_node_height(self.raw.as_ptr()) },
            depth: unsafe { sys::synctex_node_depth(self.raw.as_ptr()) },
        }
    }

    pub fn tex_box(&self) -> TexBox {
        TexBox {
            h: unsafe { sys::synctex_node_box_h(self.raw.as_ptr()) },
            v: unsafe { sys::synctex_node_box_v(self.raw.as_ptr()) },
            width: unsafe { sys::synctex_node_box_width(self.raw.as_ptr()) },
            height: unsafe { sys::synctex_node_box_height(self.raw.as_ptr()) },
            depth: unsafe { sys::synctex_node_box_depth(self.raw.as_ptr()) },
        }
    }

    pub fn log_debug(&self) {
        unsafe { sys::synctex_node_log(self.raw.as_ptr()) };
    }

    pub fn display_debug(&self) {
        unsafe { sys::synctex_node_display(self.raw.as_ptr()) };
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleBox {
    pub h: f32,
    pub v: f32,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TexBox {
    pub h: i32,
    pub v: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}

fn status_to_result(status: sys::synctex_status_t) -> Result<usize> {
    if status < 0 {
        Err(Error::Status(status))
    } else {
        Ok(status as usize)
    }
}

fn path_to_cstring(path: &Path) -> Result<CString> {
    let path = path.to_str().ok_or(Error::InvalidPath)?;
    Ok(CString::new(path)?)
}

unsafe fn optional_cstr<'a>(value: *const std::os::raw::c_char) -> Option<&'a CStr> {
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
