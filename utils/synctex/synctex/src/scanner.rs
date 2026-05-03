use std::ffi::CStr;
use std::ffi::CString;
use std::marker::PhantomData;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;

use terrazzo_synctex_sys as sys;

use crate::Error;
use crate::Node;
use crate::QueryResults;
use crate::Result;
use crate::optional_cstr;
use crate::status_to_result;

/// Scanner for a SyncTeX sidecar file associated with a rendered output document.
///
/// A scanner owns the native SyncTeX scanner handle and frees it when dropped.
/// Query result nodes and borrowed strings returned by this type are valid only
/// while the scanner remains alive.
#[derive(Debug)]
pub struct Scanner {
    pub(crate) raw: NonNull<sys::synctex_scanner_t>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Scanner {
    /// Opens and parses the SyncTeX data for `output`.
    ///
    /// `build_directory` is forwarded to SyncTeX as the directory to search for
    /// generated files. Pass `None` to use the output file's location.
    pub fn open(output: &Path, build_directory: Option<&Path>) -> Result<Self> {
        Self::new_with_output_file(output, build_directory, true)
    }

    /// Creates a scanner for `output`.
    ///
    /// If `parse` is true, SyncTeX parses the data during construction.
    /// Otherwise call [`Self::parse`] before issuing queries that require parsed
    /// SyncTeX content.
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

    /// Parses the scanner's SyncTeX data.
    ///
    /// This is useful for scanners created with `parse` set to false.
    pub fn parse(&mut self) -> Result<()> {
        let raw = unsafe { sys::synctex_scanner_parse(self.raw.as_ptr()) };
        self.raw = NonNull::new(raw).ok_or(Error::ParseFailed)?;
        Ok(())
    }

    /// Runs a forward SyncTeX query from an input source position to output nodes.
    ///
    /// `input`, `line`, and `column` identify the source location. `page_hint`
    /// optionally biases the search toward a rendered page; pass `0` when no
    /// page is known.
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

    /// Runs a reverse SyncTeX query from an output page coordinate to source nodes.
    ///
    /// `h` and `v` are SyncTeX horizontal and vertical coordinates on `page`.
    pub fn edit_query(&mut self, page: i32, h: f32, v: f32) -> Result<QueryResults<'_>> {
        let status = unsafe { sys::synctex_edit_query(self.raw.as_ptr(), page, h, v) };
        QueryResults::new(self, status)
    }

    /// Resets the scanner's current query result iterator.
    pub fn reset_result(&mut self) -> Result<()> {
        status_to_result(unsafe { sys::synctex_scanner_reset_result(self.raw.as_ptr()) }).map(drop)
    }

    /// Returns the input file path registered for `tag`.
    ///
    /// SyncTeX assigns each source file an integer tag in its `Input:<tag>:<name>`
    /// records. Other nodes store that tag instead of repeating the source path.
    pub fn name_for_tag(&self, tag: i32) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_name(self.raw.as_ptr(), tag)) }
    }

    /// Returns the SyncTeX tag registered for an input file path.
    ///
    /// `name` is the source path as recorded by SyncTeX, such as the path passed
    /// to a forward query. The returned tag is the integer used by SyncTeX nodes
    /// to refer back to that input file.
    pub fn tag_for_name(&self, name: &Path) -> Result<i32> {
        let name = path_to_cstring(name)?;
        Ok(unsafe { sys::synctex_scanner_get_tag(self.raw.as_ptr(), name.as_ptr()) })
    }

    /// Returns the first input node in the parsed SyncTeX tree.
    pub fn input(&self) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_scanner_input(self.raw.as_ptr())) }
    }

    /// Returns the input node matching `tag`.
    pub fn input_with_tag(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_scanner_input_with_tag(self.raw.as_ptr(), tag)) }
    }

    /// Returns the output file path recorded by SyncTeX.
    pub fn output(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_output(self.raw.as_ptr())) }
    }

    /// Returns the SyncTeX file path used by the scanner.
    pub fn synctex(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_synctex(self.raw.as_ptr())) }
    }

    /// Returns the output format recorded by SyncTeX, such as `pdf`.
    pub fn output_format(&self) -> Option<&CStr> {
        unsafe { optional_cstr(sys::synctex_scanner_get_output_fmt(self.raw.as_ptr())) }
    }

    /// Returns the horizontal offset used to convert TeX coordinates to output coordinates.
    ///
    /// SyncTeX stores node positions in TeX coordinates. Visible output
    /// coordinates are computed as `tex_x * magnification + x_offset`.
    pub fn x_offset(&self) -> i32 {
        unsafe { sys::synctex_scanner_x_offset(self.raw.as_ptr()) }
    }

    /// Returns the vertical offset used to convert TeX coordinates to output coordinates.
    ///
    /// SyncTeX stores node positions in TeX coordinates. Visible output
    /// coordinates are computed as `tex_y * magnification + y_offset`.
    pub fn y_offset(&self) -> i32 {
        unsafe { sys::synctex_scanner_y_offset(self.raw.as_ptr()) }
    }

    /// Returns the scale factor used to convert TeX coordinates to output coordinates.
    ///
    /// SyncTeX applies this factor before adding the horizontal or vertical
    /// offset, as in `tex_x * magnification + x_offset`.
    pub fn magnification(&self) -> f32 {
        unsafe { sys::synctex_scanner_magnification(self.raw.as_ptr()) }
    }

    /// Returns the sheet node for `page`.
    pub fn sheet(&self, page: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_sheet(self.raw.as_ptr(), page)) }
    }

    /// Returns the content node for the sheet at `page`.
    pub fn sheet_content(&self, page: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_sheet_content(self.raw.as_ptr(), page)) }
    }

    /// Returns the form node identified by `tag`.
    pub fn form(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_form(self.raw.as_ptr(), tag)) }
    }

    /// Returns the content node for the form identified by `tag`.
    pub fn form_content(&self, tag: i32) -> Option<Node<'_>> {
        unsafe { Node::from_raw(sys::synctex_form_content(self.raw.as_ptr(), tag)) }
    }

    /// Writes the scanner's debug representation to standard output.
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

fn path_to_cstring(path: &Path) -> Result<CString> {
    let path = path.to_str().ok_or(Error::InvalidPath)?;
    Ok(CString::new(path)?)
}
