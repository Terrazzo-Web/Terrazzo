use terrazzo::html;
use terrazzo::prelude::*;

use super::mousemove::MousemoveManager;

terrazzo_css::import_style!(pub(super) style, "resize_bar.scss");

#[html]
pub fn resize_bar_horz(resize_manager: MousemoveManager) -> XElement {
    div(
        class = style::RESIZE_BAR_HORZ,
        #[cfg(not(feature = "client-prod"))]
        class = "resize-bar-horz",
        mousedown = resize_manager.mousedown(),
        dblclick = move |_| resize_manager.delta.set(None),
        div(div()),
    )
}

#[html]
pub fn resize_bar_vert(resize_manager: MousemoveManager) -> XElement {
    div(
        class = style::RESIZE_BAR_VERT,
        #[cfg(not(feature = "client-prod"))]
        class = "resize-bar-vert",
        mousedown = resize_manager.mousedown(),
        dblclick = move |_| resize_manager.delta.set(None),
        div(div()),
    )
}
