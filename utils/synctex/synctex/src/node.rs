use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr::NonNull;

use terrazzo_synctex_sys as sys;

use crate::Scanner;
use crate::TexBox;
use crate::VisibleBox;
use crate::optional_cstr;

#[derive(Clone, Copy, Debug)]
pub struct Node<'scanner> {
    raw: NonNull<sys::synctex_node_t>,
    _scanner: PhantomData<&'scanner Scanner>,
}

impl<'scanner> Node<'scanner> {
    pub(crate) unsafe fn from_raw(raw: sys::synctex_node_p) -> Option<Self> {
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
