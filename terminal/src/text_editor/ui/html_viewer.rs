use std::sync::Arc;

use terrazzo::html;
use terrazzo::prelude::*;

terrazzo_css::import_style!(pub(super) style, "html_viewer.scss");

#[html]
pub(super) fn html_viewer(content: Arc<str>) -> XElement {
    iframe(
        sandbox = "",
        title = "HTML preview",
        #[cfg(not(feature = "client-prod"))]
        data_viewer = "html",
        srcdoc = content.as_ref().to_owned(),
    )
}
