#![allow(non_camel_case_types)]

use std::os::raw::c_char;
use std::os::raw::c_float;
use std::os::raw::c_int;
use std::os::raw::c_long;

#[repr(C)]
pub struct synctex_scanner_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct synctex_node_t {
    _private: [u8; 0],
}

pub type synctex_scanner_p = *mut synctex_scanner_t;
pub type synctex_node_p = *mut synctex_node_t;
pub type synctex_status_t = c_long;
pub type synctex_printer_f = unsafe extern "C" fn(*const c_char, ...) -> c_int;

unsafe extern "C" {
    pub fn synctex_scanner_new_with_output_file(
        output: *const c_char,
        build_directory: *const c_char,
        parse: c_int,
    ) -> synctex_scanner_p;
    pub fn synctex_scanner_free(scanner: synctex_scanner_p) -> c_int;
    pub fn synctex_scanner_parse(scanner: synctex_scanner_p) -> synctex_scanner_p;

    pub fn synctex_display_query(
        scanner: synctex_scanner_p,
        input: *const c_char,
        line: c_int,
        column: c_int,
        page_hint: c_int,
    ) -> synctex_status_t;
    pub fn synctex_edit_query(
        scanner: synctex_scanner_p,
        page: c_int,
        h: c_float,
        v: c_float,
    ) -> synctex_status_t;
    pub fn synctex_scanner_next_result(scanner: synctex_scanner_p) -> synctex_node_p;
    pub fn synctex_scanner_reset_result(scanner: synctex_scanner_p) -> synctex_status_t;

    pub fn synctex_node_box_visible_h(node: synctex_node_p) -> c_float;
    pub fn synctex_node_box_visible_v(node: synctex_node_p) -> c_float;
    pub fn synctex_node_box_visible_width(node: synctex_node_p) -> c_float;
    pub fn synctex_node_box_visible_height(node: synctex_node_p) -> c_float;
    pub fn synctex_node_box_visible_depth(node: synctex_node_p) -> c_float;

    pub fn synctex_node_visible_h(node: synctex_node_p) -> c_float;
    pub fn synctex_node_visible_v(node: synctex_node_p) -> c_float;
    pub fn synctex_node_visible_width(node: synctex_node_p) -> c_float;
    pub fn synctex_node_visible_height(node: synctex_node_p) -> c_float;
    pub fn synctex_node_visible_depth(node: synctex_node_p) -> c_float;

    pub fn synctex_node_tag(node: synctex_node_p) -> c_int;
    pub fn synctex_node_line(node: synctex_node_p) -> c_int;
    pub fn synctex_node_mean_line(node: synctex_node_p) -> c_int;
    pub fn synctex_node_column(node: synctex_node_p) -> c_int;
    pub fn synctex_node_get_name(node: synctex_node_p) -> *const c_char;
    pub fn synctex_node_page(node: synctex_node_p) -> c_int;

    pub fn synctex_scanner_display(scanner: synctex_scanner_p);
    pub fn synctex_scanner_get_name(scanner: synctex_scanner_p, tag: c_int) -> *const c_char;
    pub fn synctex_scanner_get_tag(scanner: synctex_scanner_p, name: *const c_char) -> c_int;

    pub fn synctex_scanner_input(scanner: synctex_scanner_p) -> synctex_node_p;
    pub fn synctex_scanner_input_with_tag(scanner: synctex_scanner_p, tag: c_int)
    -> synctex_node_p;
    pub fn synctex_scanner_get_output(scanner: synctex_scanner_p) -> *const c_char;
    pub fn synctex_scanner_get_synctex(scanner: synctex_scanner_p) -> *const c_char;
    pub fn synctex_scanner_get_output_fmt(scanner: synctex_scanner_p) -> *const c_char;

    pub fn synctex_scanner_x_offset(scanner: synctex_scanner_p) -> c_int;
    pub fn synctex_scanner_y_offset(scanner: synctex_scanner_p) -> c_int;
    pub fn synctex_scanner_magnification(scanner: synctex_scanner_p) -> c_float;
    pub fn synctex_scanner_dump(
        scanner: synctex_scanner_p,
        printer: Option<synctex_printer_f>,
    ) -> c_int;

    pub fn synctex_node_parent(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_parent_sheet(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_parent_form(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_child(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_last_child(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_sibling(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_last_sibling(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_arg_sibling(node: synctex_node_p) -> synctex_node_p;
    pub fn synctex_node_next(node: synctex_node_p) -> synctex_node_p;

    pub fn synctex_sheet(scanner: synctex_scanner_p, page: c_int) -> synctex_node_p;
    pub fn synctex_sheet_content(scanner: synctex_scanner_p, page: c_int) -> synctex_node_p;
    pub fn synctex_form(scanner: synctex_scanner_p, tag: c_int) -> synctex_node_p;
    pub fn synctex_form_content(scanner: synctex_scanner_p, tag: c_int) -> synctex_node_p;

    pub fn synctex_node_log(node: synctex_node_p);
    pub fn synctex_node_display(node: synctex_node_p);

    pub fn synctex_node_h(node: synctex_node_p) -> c_int;
    pub fn synctex_node_v(node: synctex_node_p) -> c_int;
    pub fn synctex_node_width(node: synctex_node_p) -> c_int;
    pub fn synctex_node_height(node: synctex_node_p) -> c_int;
    pub fn synctex_node_depth(node: synctex_node_p) -> c_int;

    pub fn synctex_node_box_h(node: synctex_node_p) -> c_int;
    pub fn synctex_node_box_v(node: synctex_node_p) -> c_int;
    pub fn synctex_node_box_width(node: synctex_node_p) -> c_int;
    pub fn synctex_node_box_height(node: synctex_node_p) -> c_int;
    pub fn synctex_node_box_depth(node: synctex_node_p) -> c_int;
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::synctex_scanner_new_with_output_file;

    #[test]
    fn missing_output_returns_null_scanner() {
        let output = CString::new("/definitely/missing/output.pdf").unwrap();
        let scanner =
            unsafe { synctex_scanner_new_with_output_file(output.as_ptr(), std::ptr::null(), 0) };
        assert!(scanner.is_null());
    }
}
