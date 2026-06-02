use std::rc::Rc;

use terrazzo::html;
use terrazzo::prelude::*;

use super::mousemove::MousemoveManager;

terrazzo_css::import_style!(pub(super) style, "resize_bar.scss");

#[derive(Clone)]
pub struct ResizeBarProperties {
    pub dblclick: Rc<dyn Fn()>,
    pub class: Option<&'static str>,
}

impl Default for ResizeBarProperties {
    fn default() -> Self {
        Self {
            dblclick: Rc::new(|| {}),
            class: None,
        }
    }
}

#[html]
pub fn resize_bar_horz(
    resize_manager: MousemoveManager,
    properties: ResizeBarProperties,
) -> XElement {
    div(
        class = style::RESIZE_BAR_HORZ,
        class = properties.class,
        #[cfg(not(feature = "client-prod"))]
        class = "resize-bar-horz",
        mousedown = resize_manager.mousedown(),
        dblclick = move |_| {
            resize_manager.delta.set(None);
            (properties.dblclick)();
        },
        div(div()),
    )
}

#[html]
pub fn resize_bar_vert(
    resize_manager: MousemoveManager,
    properties: ResizeBarProperties,
) -> XElement {
    div(
        class = style::RESIZE_BAR_VERT,
        class = properties.class,
        #[cfg(not(feature = "client-prod"))]
        class = "resize-bar-vert",
        mousedown = resize_manager.mousedown(),
        dblclick = move |_| {
            resize_manager.delta.set(None);
            (properties.dblclick)();
        },
        div(div()),
    )
}
